//! Root-owned Unix-domain socket server for the control API.
//!
//! The server accepts at most [`MAX_CONCURRENT_CONNECTIONS`] concurrent
//! connections on a root-only Unix socket. Each connection is wrapped in a
//! [`FramedIo`] codec; a frame error poisons the connection and closes it. No
//! TCP listener is ever opened.
//!
//! Dispatch and housekeeping run on a dedicated worker thread that owns the
//! [`ServiceDispatch`] handler: request handling involves bounded blocking
//! network I/O (exit probes, Geo lookups), which must never stall the async
//! accept loop or the housekeeping cadence. Jobs are serialized through a
//! bounded channel; housekeeping uses `try_send` so a backed-up queue skips
//! a tick instead of piling up.
//!
//! Socket creation, permissions, and process lifecycle belong to the OpenWrt
//! procd adapter; this module only drives the accepted stream.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UnixListener;
use tokio::sync::{mpsc, oneshot, Semaphore};

use crate::runtime::uds::FramedIo;
use crate::service::api::{decode_request, encode_error_response, ApiRequest, ResponseEncodeError};
use crate::service::dispatch::{dispatch, ServiceDispatch};

/// Maximum concurrent control connections permitted by the frozen API.
pub const MAX_CONCURRENT_CONNECTIONS: usize = 2;

/// Bounded job queue feeding the worker thread. Backpressure applies to
/// request dispatch; housekeeping skips a tick when the queue is full.
const JOB_QUEUE_CAPACITY: usize = 32;

enum Job {
    Refresh,
    Dispatch(
        ApiRequest,
        oneshot::Sender<Result<Vec<u8>, ResponseEncodeError>>,
    ),
}

/// A control-API server bound to a Unix-domain socket.
pub struct ControlServer<S: ServiceDispatch> {
    handler: S,
}

impl<S: ServiceDispatch + Send + 'static> ControlServer<S> {
    pub const fn new(handler: S) -> Self {
        Self { handler }
    }

    /// Accept and serve connections until the listener is closed, while
    /// running periodic housekeeping (`refresh_periodic`) every
    /// `refresh_interval`; implementations decide whether the selected
    /// location source permits an exit/IP check.
    ///
    /// The handler moves to a worker thread; every job (request dispatch and
    /// housekeeping) is executed there, so bounded blocking probe/Geo I/O can
    /// no longer wedge the accept loop or a concurrent control call.
    ///
    /// At most one housekeeping job exists in the system (queued or running):
    /// the ticker re-arms only after the worker finished the previous one.
    /// A failing exit probe blocks the worker for its whole timeout, and a
    /// 10s cadence feeding 15s+ jobs once starved control requests behind a
    /// full refresh backlog.
    pub async fn serve(self, listener: UnixListener, refresh_interval: Duration) {
        let (job_tx, job_rx) = mpsc::channel::<Job>(JOB_QUEUE_CAPACITY);
        let refresh_in_flight = Arc::new(AtomicBool::new(false));
        let worker_gate = Arc::clone(&refresh_in_flight);
        let ticker_gate = Arc::clone(&refresh_in_flight);
        std::thread::Builder::new()
            .name("wloc-control".to_string())
            .spawn(move || {
                let mut handler = self.handler;
                let mut job_rx = job_rx;
                while let Some(job) = job_rx.blocking_recv() {
                    match job {
                        Job::Refresh => {
                            handler.refresh_periodic();
                            worker_gate.store(false, Ordering::Release);
                        }
                        Job::Dispatch(request, reply) => {
                            let _ = reply.send(dispatch(&request, &mut handler));
                        }
                    }
                }
            })
            .expect("wloc control worker thread must spawn");

        let connections = std::sync::Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));
        let mut ticker = tokio::time::interval(refresh_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    // Housekeeping is skippable and never overlaps itself: a
                    // queued-or-running refresh makes this tick a no-op.
                    if ticker_gate
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_err()
                    {
                        continue;
                    }
                    if job_tx.try_send(Job::Refresh).is_err() {
                        ticker_gate.store(false, Ordering::Release);
                    }
                }
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, _)) => {
                            let permit = std::sync::Arc::clone(&connections)
                                .acquire_owned()
                                .await;
                            let tx = job_tx.clone();
                            tokio::spawn(async move {
                                let _permit = permit;
                                handle_connection(stream, tx).await;
                            });
                        }
                        Err(error) => {
                            eprintln!("wloc control socket accept failed: {error}");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        }
    }
}

async fn handle_connection(stream: tokio::net::UnixStream, job_tx: mpsc::Sender<Job>) {
    let mut framed = FramedIo::new(stream);
    loop {
        let request_frame = match framed.read_frame().await {
            Ok(frame) => frame,
            Err(_) => break,
        };
        let response = match decode_request(&request_frame) {
            Ok(request) => {
                let (reply_tx, reply_rx) = oneshot::channel();
                if job_tx.send(Job::Dispatch(request, reply_tx)).await.is_err() {
                    break;
                }
                match reply_rx.await {
                    Ok(response) => response,
                    Err(_) => break,
                }
            }
            Err(code) => encode_error_response("", code),
        };
        let response_bytes = match response {
            Ok(bytes) => bytes,
            Err(_) => break,
        };
        if framed.write_frame(&response_bytes).await.is_err() {
            break;
        }
    }
}

/// Decode a single request frame for inspection without a running server.
///
/// This is primarily a testing aid; production code uses [`ControlServer`].
pub fn decode_frame(frame: &[u8]) -> Result<ApiRequest, crate::service::api::ApiErrorCode> {
    decode_request(frame)
}
