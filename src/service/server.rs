//! Root-owned Unix-domain socket server for the control API.
//!
//! The server accepts at most [`MAX_CONCURRENT_CONNECTIONS`] sequential
//! connections on a root-only Unix socket. Each connection is wrapped in a
//! [`FramedIo`] codec; a frame error poisons the connection and closes it. No
//! TCP listener is ever opened.
//!
//! Socket creation, permissions, and process lifecycle belong to the OpenWrt
//! procd adapter; this module only drives the accepted stream.

use std::time::Duration;

use tokio::net::UnixListener;

use crate::runtime::uds::FramedIo;
use crate::service::api::{decode_request, encode_error_response, ApiRequest};
use crate::service::dispatch::{dispatch, ServiceDispatch};

/// Maximum concurrent control connections permitted by the frozen API.
pub const MAX_CONCURRENT_CONNECTIONS: usize = 2;

/// A control-API server bound to a Unix-domain socket.
///
/// Connections are handled sequentially: accept, process one or more framed
/// request/response pairs until the connection poisons or closes, then accept
/// the next. This keeps the active connection count within the limit without
/// extra synchronization.
pub struct ControlServer<S: ServiceDispatch> {
    handler: S,
}

impl<S: ServiceDispatch> ControlServer<S> {
    pub const fn new(handler: S) -> Self {
        Self { handler }
    }

    /// Accept and serve connections until the listener is closed, while
    /// running periodic housekeeping (`refresh_periodic`) every
    /// `refresh_interval`; implementations decide whether the selected
    /// location source permits an exit/IP check.
    pub async fn serve(mut self, listener: UnixListener, refresh_interval: Duration) {
        let mut ticker = tokio::time::interval(refresh_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    self.handler.refresh_periodic();
                }
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, _)) => self.handle_connection(stream).await,
                        Err(error) => {
                            eprintln!("wloc control socket accept failed: {error}");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            }
        }
    }

    async fn handle_connection(&mut self, stream: tokio::net::UnixStream) {
        let mut framed = FramedIo::new(stream);
        loop {
            let request_frame = match framed.read_frame().await {
                Ok(frame) => frame,
                Err(_) => break,
            };
            let response = match decode_request(&request_frame) {
                Ok(request) => dispatch(&request, &mut self.handler),
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
}

/// Decode a single request frame for inspection without a running server.
///
/// This is primarily a testing aid; production code uses [`ControlServer`].
pub fn decode_frame(frame: &[u8]) -> Result<ApiRequest, crate::service::api::ApiErrorCode> {
    decode_request(frame)
}
