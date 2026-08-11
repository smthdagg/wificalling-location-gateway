//! Request routing for the control API.
//!
//! The dispatcher takes a decoded [`ApiRequest`] and a service handler, routes
//! the request to the matching method, and returns a fully encoded, bounded
//! response frame. Handler failures map to stable error envelopes; success
//! wraps the result payload. No device, location, provider, or credential
//! material is added by the dispatcher.

use serde_json::Value;

use super::api::{
    encode_error_parts, encode_result_response, ApiMethod, ApiRequest, ResponseEncodeError,
};

/// Runtime failures surfaced by service handlers. Each variant maps to a
/// stable wire code, component, and retryable flag in the response envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchError {
    /// Configuration or safety scope rejected the operation.
    InvalidConfig,
    /// The engine reported it is not healthy.
    EngineUnhealthy,
    /// A redirect could not be removed and is still present.
    RedirectPresent,
    /// Cleanup after a failed enable left the service in an unsafe state.
    CleanupUnsafe,
    /// An underlying runtime adapter failed.
    RuntimeFailure,
    /// The requested information is not currently available.
    Unavailable,
}

impl DispatchError {
    pub const fn wire_code(self) -> &'static str {
        match self {
            Self::InvalidConfig => "invalid_config",
            Self::EngineUnhealthy => "engine_unhealthy",
            Self::RedirectPresent => "redirect_present",
            Self::CleanupUnsafe => "cleanup_unsafe",
            Self::RuntimeFailure => "runtime_failure",
            Self::Unavailable => "unavailable",
        }
    }

    pub const fn component(self) -> &'static str {
        match self {
            Self::EngineUnhealthy | Self::RuntimeFailure => "engine",
            Self::RedirectPresent => "network",
            Self::InvalidConfig | Self::CleanupUnsafe | Self::Unavailable => "service",
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::EngineUnhealthy | Self::RuntimeFailure | Self::Unavailable
        )
    }
}

/// Abstract service handlers behind the four v1 control methods. Production
/// adapters implement this against the real OpenWrt runtime; tests use mocks.
pub trait ServiceDispatch {
    /// Return a coordinate-free status snapshot as a JSON value.
    fn status(&mut self) -> Result<Value, DispatchError>;
    /// Start interception behind the transactional safety ordering.
    fn enable(&mut self) -> Result<(), DispatchError>;
    /// Withdraw the redirect and stop the engine.
    fn disable(&mut self) -> Result<(), DispatchError>;
    /// Reload configuration without changing the redirect state.
    fn reload(&mut self) -> Result<(), DispatchError>;
}

/// Route a decoded request to its handler and return an encoded response frame.
///
/// Success wraps the handler result (an empty object for control methods);
/// failure maps to a bounded error envelope. The response always echoes the
/// request id and carries exactly one of `result` or `error`.
pub fn dispatch(
    request: &ApiRequest,
    service: &mut impl ServiceDispatch,
) -> Result<Vec<u8>, ResponseEncodeError> {
    let request_id = request.request_id();
    match request.method() {
        ApiMethod::StatusGet => match service.status() {
            Ok(value) => encode_result_response(request_id, &value),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::ControlEnable => match service.enable() {
            Ok(()) => {
                encode_result_response(request_id, &serde_json::Value::Object(Default::default()))
            }
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::ControlDisable => match service.disable() {
            Ok(()) => {
                encode_result_response(request_id, &serde_json::Value::Object(Default::default()))
            }
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::ControlReload => match service.reload() {
            Ok(()) => {
                encode_result_response(request_id, &serde_json::Value::Object(Default::default()))
            }
            Err(error) => encode_dispatch_error(request_id, error),
        },
    }
}

fn encode_dispatch_error(
    request_id: &str,
    error: DispatchError,
) -> Result<Vec<u8>, ResponseEncodeError> {
    encode_error_parts(
        request_id,
        error.wire_code(),
        error.component(),
        error.retryable(),
    )
}
