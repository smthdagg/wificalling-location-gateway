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
use crate::service::api::{
    decode_request, decode_v2_profile_request, encode_error_response, encode_v2_error_response,
    ApiRequest, SERVICE_API_V2_ID,
};
use crate::service::dispatch::{
    dispatch, dispatch_v2, DispatchError, ProfileDispatch, ServiceDispatch,
};

/// Maximum concurrent control connections permitted by the frozen API.
pub const MAX_CONCURRENT_CONNECTIONS: usize = 2;

/// A control-API server bound to a Unix-domain socket.
///
/// Connections are handled sequentially: accept, process one or more framed
/// request/response pairs until the connection poisons or closes, then accept
/// the next. This keeps the active connection count within the limit without
/// extra synchronization.
pub struct ControlServer<S: ServiceDispatch, P: ProfileDispatch = NoProfileDispatch> {
    handler: S,
    profiles: P,
}

/// Default v2 adapter used by the legacy constructor. It makes v2 available
/// without changing v1 callers, but never invents profiles.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoProfileDispatch;

impl ProfileDispatch for NoProfileDispatch {
    fn list_profiles(&mut self) -> Result<serde_json::Value, DispatchError> {
        Err(DispatchError::Unavailable)
    }

    fn get_profile(&mut self, _profile_id: &str) -> Result<serde_json::Value, DispatchError> {
        Err(DispatchError::Unavailable)
    }

    fn create_profile(
        &mut self,
        _params: &crate::service::api::ProfileRequestParams,
    ) -> Result<serde_json::Value, DispatchError> {
        Err(DispatchError::Unavailable)
    }

    fn update_profile(
        &mut self,
        _profile_id: &str,
        _params: &crate::service::api::ProfileRequestParams,
    ) -> Result<serde_json::Value, DispatchError> {
        Err(DispatchError::Unavailable)
    }

    fn delete_profile(&mut self, _profile_id: &str) -> Result<serde_json::Value, DispatchError> {
        Err(DispatchError::Unavailable)
    }
}

impl<S: ServiceDispatch> ControlServer<S, NoProfileDispatch> {
    pub const fn new(handler: S) -> Self {
        Self {
            handler,
            profiles: NoProfileDispatch,
        }
    }
}

impl<S: ServiceDispatch, P: ProfileDispatch> ControlServer<S, P> {
    pub const fn with_profile_dispatch(handler: S, profiles: P) -> Self {
        Self { handler, profiles }
    }

    /// Accept and serve connections until the listener is closed, while
    /// running periodic housekeeping (`refresh_periodic`) every
    /// `refresh_interval` so the status file stays fresh without API traffic.
    pub async fn serve(mut self, listener: UnixListener, refresh_interval: Duration) {
        let mut ticker = tokio::time::interval(refresh_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    self.handler.refresh_periodic();
                }
                accepted = listener.accept() => {
                    if let Ok((stream, _)) = accepted {
                        self.handle_connection(stream).await;
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
            let response = if is_v2_frame(&request_frame) {
                match decode_v2_profile_request(&request_frame) {
                    Ok(request) => dispatch_v2(&request, &mut self.profiles),
                    Err(code) => encode_v2_error_response("", code),
                }
            } else {
                match decode_request(&request_frame) {
                    Ok(request) => dispatch(&request, &mut self.handler),
                    Err(code) => encode_error_response("", code),
                }
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

fn is_v2_frame(frame: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(frame)
        .ok()
        .and_then(|value| {
            value
                .get("api_version")
                .and_then(serde_json::Value::as_str)
                .map(|version| version == SERVICE_API_V2_ID)
        })
        .unwrap_or(false)
}

/// Decode a single request frame for inspection without a running server.
///
/// This is primarily a testing aid; production code uses [`ControlServer`].
pub fn decode_frame(frame: &[u8]) -> Result<ApiRequest, crate::service::api::ApiErrorCode> {
    decode_request(frame)
}
