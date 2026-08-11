//! Strict, size-bounded control API decoder and response encoder.
//!
//! Transport is intentionally out of scope here. The OpenWrt adapter will
//! expose this contract only through a root-owned local Unix socket or facade.

use std::fmt;

use serde::{Deserialize, Serialize};

pub const SERVICE_API_ID: &str = "wloc.service/v1";
/// Single source of truth for the control-frame size bound. Re-exported from
/// the transport codec so the API decoder and the frame layer cannot drift.
pub use crate::runtime::uds::MAX_CONTROL_FRAME_BYTES;
const MAX_REQUEST_ID_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiMethod {
    StatusGet,
    ControlEnable,
    ControlDisable,
    ControlReload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiErrorCode {
    FrameTooLarge,
    MalformedRequest,
    IncompatibleVersion,
    InvalidRequestId,
    UnknownMethod,
}

impl ApiErrorCode {
    /// Stable snake_case wire code for the response error envelope. Adding or
    /// renaming a code requires a new reviewed contract test.
    pub const fn wire_code(self) -> &'static str {
        match self {
            Self::FrameTooLarge => "frame_too_large",
            Self::MalformedRequest => "malformed_request",
            Self::IncompatibleVersion => "incompatible_version",
            Self::InvalidRequestId => "invalid_request_id",
            Self::UnknownMethod => "unknown_method",
        }
    }

    /// Decode errors are never retryable: the caller must fix the request
    /// before retransmitting. Runtime failures use a separate error path.
    pub const fn retryable(self) -> bool {
        false
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ApiRequest {
    api_version: String,
    request_id: String,
    method: ApiMethod,
}

impl ApiRequest {
    pub fn api_version(&self) -> &str {
        &self.api_version
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub const fn method(&self) -> ApiMethod {
        self.method
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest {
    api_version: String,
    request_id: String,
    method: String,
    params: EmptyParams,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyParams {}

pub fn decode_request(frame: &[u8]) -> Result<ApiRequest, ApiErrorCode> {
    if frame.is_empty() {
        return Err(ApiErrorCode::MalformedRequest);
    }
    if frame.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(ApiErrorCode::FrameTooLarge);
    }

    let wire: WireRequest =
        serde_json::from_slice(frame).map_err(|_| ApiErrorCode::MalformedRequest)?;
    if wire.api_version != SERVICE_API_ID {
        return Err(ApiErrorCode::IncompatibleVersion);
    }
    if !valid_request_id(&wire.request_id) {
        return Err(ApiErrorCode::InvalidRequestId);
    }
    let method = match wire.method.as_str() {
        "status.get" => ApiMethod::StatusGet,
        "control.enable" => ApiMethod::ControlEnable,
        "control.disable" => ApiMethod::ControlDisable,
        "control.reload" => ApiMethod::ControlReload,
        _ => return Err(ApiErrorCode::UnknownMethod),
    };
    let EmptyParams {} = wire.params;

    Ok(ApiRequest {
        api_version: wire.api_version,
        request_id: wire.request_id,
        method,
    })
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// Failure to encode a response frame within the transport bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseEncodeError {
    /// The serialized response would exceed [`MAX_CONTROL_FRAME_BYTES`].
    Oversized,
}

impl fmt::Display for ResponseEncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized => formatter.write_str("control response exceeds the frame bound"),
        }
    }
}

impl std::error::Error for ResponseEncodeError {}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    component: &'static str,
    retryable: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    api_version: &'static str,
    request_id: String,
    error: ErrorBody,
}

#[derive(Serialize)]
struct ResultResponse {
    api_version: &'static str,
    request_id: String,
    result: serde_json::Value,
}

/// Encode a bounded error response frame. The envelope carries the stable
/// wire code, a fixed `service` component, and the non-retryable flag. It
/// never includes a `result` field, device material, or provider payload.
///
/// `request_id` may be empty when the request could not be parsed at all.
pub fn encode_error_response(
    request_id: &str,
    code: ApiErrorCode,
) -> Result<Vec<u8>, ResponseEncodeError> {
    let response = ErrorResponse {
        api_version: SERVICE_API_ID,
        request_id: request_id.to_owned(),
        error: ErrorBody {
            code: code.wire_code(),
            component: "service",
            retryable: code.retryable(),
        },
    };
    let bytes = serde_json::to_vec(&response).map_err(|_| ResponseEncodeError::Oversized)?;
    if bytes.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(ResponseEncodeError::Oversized);
    }
    Ok(bytes)
}

/// Encode a bounded result response frame wrapping a pre-serialized result
/// value. The envelope echoes the request id and never includes an `error`
/// field. A result whose envelope would exceed the frame bound is rejected.
pub fn encode_result_response(
    request_id: &str,
    result: &serde_json::Value,
) -> Result<Vec<u8>, ResponseEncodeError> {
    let response = ResultResponse {
        api_version: SERVICE_API_ID,
        request_id: request_id.to_owned(),
        result: result.clone(),
    };
    let bytes = serde_json::to_vec(&response).map_err(|_| ResponseEncodeError::Oversized)?;
    if bytes.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(ResponseEncodeError::Oversized);
    }
    Ok(bytes)
}
