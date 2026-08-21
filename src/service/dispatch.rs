//! Request routing for the control API.
//!
//! The dispatcher takes a decoded [`ApiRequest`] and a service handler, routes
//! the request to the matching method, and returns a fully encoded, bounded
//! response frame. Handler failures map to stable error envelopes; success
//! wraps the result payload. No device, location, provider, or credential
//! material is added by the dispatcher.

use serde_json::{json, Value};

use super::api::{
    encode_error_parts, encode_result_response, encode_v2_error_parts, encode_v2_result_response,
    ApiMethod, ApiRequest, ApiV2ProfileRequest, ProfileApiMethod, ProfileRequestParams,
    RequestParams, ResponseEncodeError,
};
use crate::config::{DeviceProfile, LocationMode, NodeSelectionMode, ProfileModel};

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
    /// A manual location could not be resolved or is out of range.
    InvalidLocation,
    /// The requested profile does not exist in the selected runtime store.
    ProfileNotFound,
    /// A create operation would reuse an existing profile id.
    ProfileAlreadyExists,
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
            Self::InvalidLocation => "invalid_location",
            Self::ProfileNotFound => "profile_not_found",
            Self::ProfileAlreadyExists => "profile_already_exists",
        }
    }

    pub const fn component(self) -> &'static str {
        match self {
            Self::EngineUnhealthy | Self::RuntimeFailure => "engine",
            Self::RedirectPresent => "network",
            Self::InvalidConfig
            | Self::CleanupUnsafe
            | Self::Unavailable
            | Self::InvalidLocation => "service",
            Self::ProfileNotFound | Self::ProfileAlreadyExists => "service",
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::EngineUnhealthy | Self::RuntimeFailure | Self::Unavailable
        )
    }
}

/// Controlled adapter for v2 profile CRUD. Implementations own the runtime
/// representation; the dispatcher only passes already validated API params.
pub trait ProfileDispatch {
    fn list_profiles(&mut self) -> Result<Value, DispatchError>;
    fn get_profile(&mut self, profile_id: &str) -> Result<Value, DispatchError>;
    fn create_profile(&mut self, params: &ProfileRequestParams) -> Result<Value, DispatchError>;
    fn update_profile(
        &mut self,
        profile_id: &str,
        params: &ProfileRequestParams,
    ) -> Result<Value, DispatchError>;
    fn delete_profile(&mut self, profile_id: &str) -> Result<Value, DispatchError>;
}

/// Bounded in-memory profile adapter for the daemon control plane. It starts
/// empty and never invents a default profile or stores node credentials.
#[derive(Clone, Debug)]
pub struct InMemoryProfileStore {
    model: ProfileModel,
}

impl Default for InMemoryProfileStore {
    fn default() -> Self {
        Self {
            model: ProfileModel::new(Vec::new()).expect("empty profile model is valid"),
        }
    }
}

impl InMemoryProfileStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_profiles(profiles: Vec<DeviceProfile>) -> Result<Self, DispatchError> {
        ProfileModel::new(profiles)
            .map(|model| Self { model })
            .map_err(|_| DispatchError::InvalidConfig)
    }

    pub fn model(&self) -> &ProfileModel {
        &self.model
    }

    fn redacted_profile(&self, profile_id: &str) -> Result<Value, DispatchError> {
        let profiles = self
            .model
            .redacted_status()
            .map_err(|_| DispatchError::RuntimeFailure)?;
        profiles
            .into_iter()
            .find(|profile| profile.get("profile_id").and_then(Value::as_str) == Some(profile_id))
            .ok_or(DispatchError::ProfileNotFound)
    }

    fn replace_model(&mut self, profiles: Vec<DeviceProfile>) -> Result<(), DispatchError> {
        self.model
            .replace(profiles)
            .map_err(|_| DispatchError::InvalidConfig)
    }
}

impl ProfileDispatch for InMemoryProfileStore {
    fn list_profiles(&mut self) -> Result<Value, DispatchError> {
        Ok(
            json!({"profiles": self.model.redacted_status().map_err(|_| DispatchError::RuntimeFailure)?}),
        )
    }

    fn get_profile(&mut self, profile_id: &str) -> Result<Value, DispatchError> {
        Ok(json!({"profile": self.redacted_profile(profile_id)?}))
    }

    fn create_profile(&mut self, params: &ProfileRequestParams) -> Result<Value, DispatchError> {
        let profile = profile_from_create(params)?;
        if self
            .model
            .profiles()
            .iter()
            .any(|item| item.id == profile.id)
        {
            return Err(DispatchError::ProfileAlreadyExists);
        }
        let mut profiles = self.model.profiles().to_vec();
        let profile_id = profile.id.clone();
        profiles.push(profile);
        self.replace_model(profiles)?;
        Ok(json!({"profile_id": profile_id}))
    }

    fn update_profile(
        &mut self,
        profile_id: &str,
        params: &ProfileRequestParams,
    ) -> Result<Value, DispatchError> {
        let mut profiles = self.model.profiles().to_vec();
        let profile = profiles
            .iter_mut()
            .find(|item| item.id == profile_id)
            .ok_or(DispatchError::ProfileNotFound)?;
        apply_profile_update(profile, params)?;
        self.replace_model(profiles)?;
        Ok(json!({"profile_id": profile_id}))
    }

    fn delete_profile(&mut self, profile_id: &str) -> Result<Value, DispatchError> {
        let original_len = self.model.profiles().len();
        let profiles: Vec<_> = self
            .model
            .profiles()
            .iter()
            .filter(|profile| profile.id != profile_id)
            .cloned()
            .collect();
        if profiles.len() == original_len {
            return Err(DispatchError::ProfileNotFound);
        }
        self.replace_model(profiles)?;
        Ok(json!({"profile_id": profile_id}))
    }
}

fn profile_from_create(params: &ProfileRequestParams) -> Result<DeviceProfile, DispatchError> {
    let (
        Some(id),
        Some(label),
        Some(assigned_device),
        Some(node_ref),
        Some(node_mode),
        Some(geo_source),
        Some(enabled),
    ) = (
        params.profile_id.clone(),
        params.label.clone(),
        params.assigned_device.clone(),
        params.node_ref.clone(),
        params.node_mode.as_deref(),
        params.geo_source.as_deref(),
        params.enabled,
    )
    else {
        return Err(DispatchError::InvalidConfig);
    };
    Ok(DeviceProfile {
        id,
        label,
        assigned_device: Some(assigned_device),
        node_ref,
        node_mode: parse_node_mode(node_mode)?,
        location_mode: parse_location_mode(geo_source)?,
        manual_latitude: params.manual_latitude,
        manual_longitude: params.manual_longitude,
        manual_location_ref: params.manual_location_ref.clone(),
        enabled,
    })
}

fn apply_profile_update(
    profile: &mut DeviceProfile,
    params: &ProfileRequestParams,
) -> Result<(), DispatchError> {
    if let Some(label) = params.label.clone() {
        profile.label = label;
    }
    if let Some(assigned_device) = params.assigned_device.clone() {
        profile.assigned_device = Some(assigned_device);
    }
    if let Some(node_ref) = params.node_ref.clone() {
        profile.node_ref = node_ref;
    }
    if let Some(node_mode) = params.node_mode.as_deref() {
        profile.node_mode = parse_node_mode(node_mode)?;
    }
    if let Some(geo_source) = params.geo_source.as_deref() {
        profile.location_mode = parse_location_mode(geo_source)?;
    }
    if params.manual_latitude.is_some() {
        profile.manual_latitude = params.manual_latitude;
        profile.manual_longitude = params.manual_longitude;
    }
    if let Some(reference) = params.manual_location_ref.clone() {
        profile.manual_location_ref = Some(reference);
    }
    if let Some(enabled) = params.enabled {
        profile.enabled = enabled;
    }
    Ok(())
}

fn parse_node_mode(value: &str) -> Result<NodeSelectionMode, DispatchError> {
    match value {
        "fixed" => Ok(NodeSelectionMode::Fixed),
        "gateway_default" => Ok(NodeSelectionMode::GatewayDefault),
        _ => Err(DispatchError::InvalidConfig),
    }
}

fn parse_location_mode(value: &str) -> Result<LocationMode, DispatchError> {
    match value {
        "auto" => Ok(LocationMode::Auto),
        "manual" => Ok(LocationMode::Manual),
        _ => Err(DispatchError::InvalidConfig),
    }
}

/// Abstract service handlers behind the control methods. Production adapters
/// implement this against the real OpenWrt runtime; tests use mocks.
pub trait ServiceDispatch {
    /// Admit enabled profile redirects after the shared proxy/listener
    /// readiness gate. Singleton handlers keep the no-op default.
    fn activate_profiles(&mut self) {}

    /// Return a coordinate-free status snapshot as a JSON value.
    fn status(&mut self) -> Result<Value, DispatchError>;
    /// Start interception behind the transactional safety ordering.
    fn enable(&mut self) -> Result<(), DispatchError>;
    /// Withdraw the redirect and stop the engine.
    fn disable(&mut self) -> Result<(), DispatchError>;
    /// Reload configuration without changing the redirect state.
    fn reload(&mut self) -> Result<(), DispatchError>;
    /// Apply a manual location preset (by place query or explicit coordinates).
    fn set_manual_location(&mut self, params: &RequestParams) -> Result<(), DispatchError>;
    /// Return to automatic node-following location.
    fn clear_manual_location(&mut self) -> Result<(), DispatchError>;
    /// Geocode a place query and return the city name and coordinates
    /// without applying them.
    fn search_location(&mut self, query: &str) -> Result<Value, DispatchError>;
    /// Periodic housekeeping: refresh probe/geo evidence and rewrite the
    /// status file so the monitor page stays fresh without API traffic.
    /// The default is a no-op so lightweight handlers are unaffected.
    fn refresh_periodic(&mut self) {}
    /// Force an immediate exit/geo re-probe, discarding cached evidence.
    /// The default is a no-op so handlers without a probe are unaffected.
    fn refresh_evidence(&mut self) -> Result<(), DispatchError> {
        Ok(())
    }
}

impl ServiceDispatch for Box<dyn ServiceDispatch> {
    fn activate_profiles(&mut self) {
        (**self).activate_profiles();
    }

    fn status(&mut self) -> Result<Value, DispatchError> {
        (**self).status()
    }

    fn enable(&mut self) -> Result<(), DispatchError> {
        (**self).enable()
    }

    fn disable(&mut self) -> Result<(), DispatchError> {
        (**self).disable()
    }

    fn reload(&mut self) -> Result<(), DispatchError> {
        (**self).reload()
    }

    fn set_manual_location(&mut self, params: &RequestParams) -> Result<(), DispatchError> {
        (**self).set_manual_location(params)
    }

    fn clear_manual_location(&mut self) -> Result<(), DispatchError> {
        (**self).clear_manual_location()
    }

    fn search_location(&mut self, query: &str) -> Result<Value, DispatchError> {
        (**self).search_location(query)
    }

    fn refresh_periodic(&mut self) {
        (**self).refresh_periodic();
    }

    fn refresh_evidence(&mut self) -> Result<(), DispatchError> {
        (**self).refresh_evidence()
    }
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
    let empty =
        || encode_result_response(request_id, &serde_json::Value::Object(Default::default()));
    match request.method() {
        ApiMethod::StatusGet => match service.status() {
            Ok(value) => encode_result_response(request_id, &value),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::ControlEnable => match service.enable() {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::ControlDisable => match service.disable() {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::ControlReload => match service.reload() {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::GeoSet => match service.set_manual_location(request.params()) {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::GeoClear => match service.clear_manual_location() {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::GeoSearch => match request.params().query.as_deref() {
            Some(query) if !query.trim().is_empty() => {
                match service.search_location(query.trim()) {
                    Ok(value) => encode_result_response(request_id, &value),
                    Err(error) => encode_dispatch_error(request_id, error),
                }
            }
            _ => encode_dispatch_error(request_id, DispatchError::InvalidLocation),
        },
        ApiMethod::Refresh => match service.refresh_evidence() {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
    }
}

/// Route a decoded v2 profile request to a controlled profile adapter and
/// return the bounded v2 response envelope.
pub fn dispatch_v2(
    request: &ApiV2ProfileRequest,
    profiles: &mut impl ProfileDispatch,
) -> Result<Vec<u8>, ResponseEncodeError> {
    let request_id = request.request_id();
    let result = match request.method() {
        ProfileApiMethod::List => profiles.list_profiles(),
        ProfileApiMethod::Get => request
            .params()
            .profile_id
            .as_deref()
            .ok_or(DispatchError::InvalidConfig)
            .and_then(|profile_id| profiles.get_profile(profile_id)),
        ProfileApiMethod::Create => profiles.create_profile(request.params()),
        ProfileApiMethod::Update => request
            .params()
            .profile_id
            .as_deref()
            .ok_or(DispatchError::InvalidConfig)
            .and_then(|profile_id| profiles.update_profile(profile_id, request.params())),
        ProfileApiMethod::Delete => request
            .params()
            .profile_id
            .as_deref()
            .ok_or(DispatchError::InvalidConfig)
            .and_then(|profile_id| profiles.delete_profile(profile_id)),
    };
    match result {
        Ok(value) => encode_v2_result_response(request_id, &value),
        Err(error) => encode_v2_dispatch_error(request_id, error),
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

fn encode_v2_dispatch_error(
    request_id: &str,
    error: DispatchError,
) -> Result<Vec<u8>, ResponseEncodeError> {
    encode_v2_error_parts(
        request_id,
        error.wire_code(),
        error.component(),
        error.retryable(),
    )
}
