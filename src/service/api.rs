//! Strict, size-bounded control API decoder and response encoder.
//!
//! Transport is intentionally out of scope here. The OpenWrt adapter will
//! expose this contract only through a root-owned local Unix socket or facade.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::config::{
    validate_device_address, validate_location_ref, validate_node_ref, validate_profile_id,
    validate_profile_label,
};

pub const SERVICE_API_ID: &str = "wloc.service/v1";
pub const SERVICE_API_V2_ID: &str = "wloc.service/v2";
/// Single source of truth for the control-frame size bound. Re-exported from
/// the transport codec so the API decoder and the frame layer cannot drift.
pub use crate::runtime::uds::MAX_CONTROL_FRAME_BYTES;
const MAX_REQUEST_ID_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileApiMethod {
    List,
    Get,
    Create,
    Update,
    Delete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiV2ErrorCode {
    FrameTooLarge,
    MalformedRequest,
    IncompatibleVersion,
    InvalidRequestId,
    UnknownMethod,
    InvalidParams,
}

impl ApiV2ErrorCode {
    pub const fn wire_code(self) -> &'static str {
        match self {
            Self::FrameTooLarge => "frame_too_large",
            Self::MalformedRequest => "malformed_request",
            Self::IncompatibleVersion => "incompatible_version",
            Self::InvalidRequestId => "invalid_request_id",
            Self::UnknownMethod => "unknown_method",
            Self::InvalidParams => "invalid_params",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileRequestParams {
    pub profile_id: Option<String>,
    pub label: Option<String>,
    pub assigned_device: Option<String>,
    pub node_ref: Option<String>,
    pub node_mode: Option<String>,
    pub geo_source: Option<String>,
    pub manual_latitude: Option<f64>,
    pub manual_longitude: Option<f64>,
    pub manual_location_ref: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApiV2ProfileRequest {
    api_version: String,
    request_id: String,
    method: ProfileApiMethod,
    params: ProfileRequestParams,
}

impl ApiV2ProfileRequest {
    pub fn api_version(&self) -> &str {
        &self.api_version
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub const fn method(&self) -> ProfileApiMethod {
        self.method
    }

    pub fn params(&self) -> &ProfileRequestParams {
        &self.params
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireV2ProfileRequest {
    api_version: String,
    request_id: String,
    method: String,
    #[serde(default)]
    params: WireV2ProfileParams,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct WireV2ProfileParams {
    #[serde(default)]
    profile_id: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    assigned_device: Option<String>,
    #[serde(default)]
    node_ref: Option<String>,
    #[serde(default)]
    node_mode: Option<String>,
    #[serde(default)]
    geo_source: Option<String>,
    #[serde(default)]
    manual_lat: Option<f64>,
    #[serde(default)]
    manual_lon: Option<f64>,
    #[serde(default)]
    manual_location_ref: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
}

impl From<WireV2ProfileParams> for ProfileRequestParams {
    fn from(params: WireV2ProfileParams) -> Self {
        Self {
            profile_id: params.profile_id,
            label: params.label,
            assigned_device: params.assigned_device,
            node_ref: params.node_ref,
            node_mode: params.node_mode,
            geo_source: params.geo_source,
            manual_latitude: params.manual_lat,
            manual_longitude: params.manual_lon,
            manual_location_ref: params.manual_location_ref,
            enabled: params.enabled,
        }
    }
}

/// Decode profile-management requests for the additive v2 API. Runtime
/// dispatch is intentionally a later issue; decoding and validation are
/// complete before any handler can receive the request.
pub fn decode_v2_profile_request(frame: &[u8]) -> Result<ApiV2ProfileRequest, ApiV2ErrorCode> {
    if frame.is_empty() {
        return Err(ApiV2ErrorCode::MalformedRequest);
    }
    if frame.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(ApiV2ErrorCode::FrameTooLarge);
    }
    let wire: WireV2ProfileRequest =
        serde_json::from_slice(frame).map_err(|_| ApiV2ErrorCode::MalformedRequest)?;
    if wire.api_version != SERVICE_API_V2_ID {
        return Err(ApiV2ErrorCode::IncompatibleVersion);
    }
    if !valid_request_id(&wire.request_id) {
        return Err(ApiV2ErrorCode::InvalidRequestId);
    }
    let method = match wire.method.as_str() {
        "profile.list" => ProfileApiMethod::List,
        "profile.get" => ProfileApiMethod::Get,
        "profile.create" => ProfileApiMethod::Create,
        "profile.update" => ProfileApiMethod::Update,
        "profile.delete" => ProfileApiMethod::Delete,
        _ => return Err(ApiV2ErrorCode::UnknownMethod),
    };
    if let Some(profile_id) = wire.params.profile_id.as_deref() {
        validate_profile_id(profile_id).map_err(|_| ApiV2ErrorCode::InvalidParams)?;
    }
    if matches!(
        method,
        ProfileApiMethod::Get | ProfileApiMethod::Update | ProfileApiMethod::Delete
    ) && wire.params.profile_id.is_none()
    {
        return Err(ApiV2ErrorCode::InvalidParams);
    }
    validate_v2_profile_params(&wire.params)?;
    Ok(ApiV2ProfileRequest {
        api_version: wire.api_version,
        request_id: wire.request_id,
        method,
        params: wire.params.into(),
    })
}

fn validate_v2_profile_params(params: &WireV2ProfileParams) -> Result<(), ApiV2ErrorCode> {
    if let Some(label) = params.label.as_deref() {
        validate_profile_label(label).map_err(|_| ApiV2ErrorCode::InvalidParams)?;
    }
    if let Some(address) = params.assigned_device.as_deref() {
        validate_device_address(address).map_err(|_| ApiV2ErrorCode::InvalidParams)?;
    }
    if let Some(node_ref) = params.node_ref.as_deref() {
        validate_node_ref(node_ref).map_err(|_| ApiV2ErrorCode::InvalidParams)?;
    }
    if let Some(node_mode) = params.node_mode.as_deref() {
        if !matches!(node_mode, "fixed" | "gateway_default") {
            return Err(ApiV2ErrorCode::InvalidParams);
        }
    }
    if let Some(geo_source) = params.geo_source.as_deref() {
        if !matches!(geo_source, "auto" | "manual") {
            return Err(ApiV2ErrorCode::InvalidParams);
        }
    }
    if let Some(reference) = params.manual_location_ref.as_deref() {
        validate_location_ref(reference).map_err(|_| ApiV2ErrorCode::InvalidParams)?;
    }
    if params.manual_lat.is_some() != params.manual_lon.is_some() {
        return Err(ApiV2ErrorCode::InvalidParams);
    }
    if let (Some(latitude), Some(longitude)) = (params.manual_lat, params.manual_lon) {
        if !latitude.is_finite()
            || !longitude.is_finite()
            || !(-90.0..=90.0).contains(&latitude)
            || !(-180.0..=180.0).contains(&longitude)
        {
            return Err(ApiV2ErrorCode::InvalidParams);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiMethod {
    StatusGet,
    ControlEnable,
    ControlDisable,
    ControlReload,
    /// Set a manual location preset (by place query or explicit coordinates).
    GeoSet,
    /// Return to automatic node-following location.
    GeoClear,
    /// Geocode a place query and return the city name and coordinates
    /// without changing the active location ("search first, apply later").
    GeoSearch,
    /// Force an immediate exit/geo re-probe, discarding cached evidence.
    /// The monitor's manual refresh button uses this so a node switch
    /// shows up without waiting for the periodic housekeeping tick.
    Refresh,
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

#[derive(Debug, PartialEq)]
pub struct ApiRequest {
    api_version: String,
    request_id: String,
    method: ApiMethod,
    params: RequestParams,
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

    pub fn params(&self) -> &RequestParams {
        &self.params
    }
}

/// Parameters for a control-API request. Only `geo.set` consumes them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RequestParams {
    /// A place query to geocode (e.g. `London, UK`).
    pub query: Option<String>,
    /// Explicit WGS84 coordinates for a manual preset.
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest {
    api_version: String,
    request_id: String,
    method: String,
    #[serde(default)]
    params: WireParams,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct WireParams {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    latitude: Option<f64>,
    #[serde(default)]
    longitude: Option<f64>,
}

impl From<WireParams> for RequestParams {
    fn from(wire: WireParams) -> Self {
        Self {
            query: wire.query,
            latitude: wire.latitude,
            longitude: wire.longitude,
        }
    }
}

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
        "geo.set" => ApiMethod::GeoSet,
        "geo.clear" => ApiMethod::GeoClear,
        "geo.search" => ApiMethod::GeoSearch,
        "control.refresh" => ApiMethod::Refresh,
        _ => return Err(ApiErrorCode::UnknownMethod),
    };

    Ok(ApiRequest {
        api_version: wire.api_version,
        request_id: wire.request_id,
        method,
        params: wire.params.into(),
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
struct ErrorBody<'a> {
    code: &'a str,
    component: &'a str,
    retryable: bool,
}

#[derive(Serialize)]
struct ErrorResponse<'a> {
    api_version: &'a str,
    request_id: String,
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct VersionedResultResponse<'a> {
    api_version: &'a str,
    request_id: String,
    result: serde_json::Value,
}

/// Encode a bounded error response frame from raw envelope parts. This is the
/// shared lower-level encoder used by both decode-error and dispatch-error
/// response paths. The envelope carries no `result` field, device material, or
/// provider payload.
///
/// `request_id` may be empty when the request could not be parsed at all.
pub fn encode_error_parts(
    request_id: &str,
    code: &str,
    component: &str,
    retryable: bool,
) -> Result<Vec<u8>, ResponseEncodeError> {
    let response = ErrorResponse {
        api_version: SERVICE_API_ID,
        request_id: request_id.to_owned(),
        error: ErrorBody {
            code,
            component,
            retryable,
        },
    };
    let bytes = serde_json::to_vec(&response).map_err(|_| ResponseEncodeError::Oversized)?;
    if bytes.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(ResponseEncodeError::Oversized);
    }
    Ok(bytes)
}

/// Encode a bounded error response frame for a decode error. The envelope
/// carries the stable wire code, a fixed `service` component, and the
/// non-retryable flag.
///
/// `request_id` may be empty when the request could not be parsed at all.
pub fn encode_error_response(
    request_id: &str,
    code: ApiErrorCode,
) -> Result<Vec<u8>, ResponseEncodeError> {
    encode_error_parts(request_id, code.wire_code(), "service", code.retryable())
}

/// Encode a bounded result response frame wrapping a pre-serialized result
/// value. The envelope echoes the request id and never includes an `error`
/// field. A result whose envelope would exceed the frame bound is rejected.
pub fn encode_result_response(
    request_id: &str,
    result: &serde_json::Value,
) -> Result<Vec<u8>, ResponseEncodeError> {
    encode_versioned_result_response(SERVICE_API_ID, request_id, result)
}

/// Encode a bounded v2 profile response. Runtime dispatch remains a later
/// issue, but UI clients can share the reviewed v1/v2 envelope shape.
pub fn encode_v2_result_response(
    request_id: &str,
    result: &serde_json::Value,
) -> Result<Vec<u8>, ResponseEncodeError> {
    encode_versioned_result_response(SERVICE_API_V2_ID, request_id, result)
}

fn encode_versioned_result_response(
    api_version: &str,
    request_id: &str,
    result: &serde_json::Value,
) -> Result<Vec<u8>, ResponseEncodeError> {
    let response = VersionedResultResponse {
        api_version,
        request_id: request_id.to_owned(),
        result: result.clone(),
    };
    let bytes = serde_json::to_vec(&response).map_err(|_| ResponseEncodeError::Oversized)?;
    if bytes.len() > MAX_CONTROL_FRAME_BYTES {
        return Err(ResponseEncodeError::Oversized);
    }
    Ok(bytes)
}
