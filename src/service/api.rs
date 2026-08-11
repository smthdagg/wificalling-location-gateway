//! Strict, size-bounded control API decoder.
//!
//! Transport is intentionally out of scope here. The OpenWrt adapter will
//! expose this contract only through a root-owned local Unix socket or facade.

use serde::Deserialize;

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
