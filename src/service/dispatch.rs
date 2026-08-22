//! Request routing for the control API.
//!
//! The dispatcher takes a decoded [`ApiRequest`] and a service handler, routes
//! the request to the matching method, and returns a fully encoded, bounded
//! response frame. Handler failures map to stable error envelopes; success
//! wraps the result payload. No device, location, provider, or credential
//! material is added by the dispatcher.

use serde_json::Value;

use super::api::{
    encode_error_parts, encode_result_response, ApiMethod, ApiRequest, RequestParams,
    ResponseEncodeError,
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
    /// A manual location could not be resolved or is out of range.
    InvalidLocation,
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
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::EngineUnhealthy | Self::RuntimeFailure | Self::Unavailable
        )
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
    fn status_for(&mut self, profile_id: Option<&str>) -> Result<Value, DispatchError> {
        let _ = profile_id;
        self.status()
    }
    /// Start interception behind the transactional safety ordering.
    fn enable(&mut self) -> Result<(), DispatchError>;
    fn enable_for(&mut self, profile_id: Option<&str>) -> Result<(), DispatchError> {
        let _ = profile_id;
        self.enable()
    }
    /// Withdraw the redirect and stop the engine.
    fn disable(&mut self) -> Result<(), DispatchError>;
    fn disable_for(&mut self, profile_id: Option<&str>) -> Result<(), DispatchError> {
        let _ = profile_id;
        self.disable()
    }
    /// Reload configuration without changing the redirect state.
    fn reload(&mut self) -> Result<(), DispatchError>;
    fn reload_for(&mut self, profile_id: Option<&str>) -> Result<(), DispatchError> {
        let _ = profile_id;
        self.reload()
    }
    /// Apply a manual location preset (by place query or explicit coordinates).
    fn set_manual_location(&mut self, params: &RequestParams) -> Result<(), DispatchError>;
    /// Return to automatic node-following location.
    fn clear_manual_location(&mut self) -> Result<(), DispatchError>;
    fn clear_manual_location_for(&mut self, profile_id: Option<&str>) -> Result<(), DispatchError> {
        let _ = profile_id;
        self.clear_manual_location()
    }
    /// Geocode a place query and return the city name and coordinates
    /// without applying them.
    fn search_location(&mut self, query: &str) -> Result<Value, DispatchError>;
    fn search_location_for(
        &mut self,
        query: &str,
        profile_id: Option<&str>,
    ) -> Result<Value, DispatchError> {
        let _ = profile_id;
        self.search_location(query)
    }
    /// Periodic housekeeping: refresh probe/geo evidence and rewrite the
    /// status file so the monitor page stays fresh without API traffic.
    /// The default is a no-op so lightweight handlers are unaffected.
    fn refresh_periodic(&mut self) {}
    /// Force an immediate exit/geo re-probe, discarding cached evidence.
    /// The default is a no-op so handlers without a probe are unaffected.
    fn refresh_evidence(&mut self) -> Result<(), DispatchError> {
        Ok(())
    }
    fn refresh_evidence_for(&mut self, profile_id: Option<&str>) -> Result<(), DispatchError> {
        let _ = profile_id;
        self.refresh_evidence()
    }
}

impl ServiceDispatch for Box<dyn ServiceDispatch> {
    fn activate_profiles(&mut self) {
        (**self).activate_profiles();
    }

    fn status(&mut self) -> Result<Value, DispatchError> {
        (**self).status()
    }

    fn status_for(&mut self, profile_id: Option<&str>) -> Result<Value, DispatchError> {
        (**self).status_for(profile_id)
    }

    fn enable(&mut self) -> Result<(), DispatchError> {
        (**self).enable()
    }

    fn enable_for(&mut self, profile_id: Option<&str>) -> Result<(), DispatchError> {
        (**self).enable_for(profile_id)
    }

    fn disable(&mut self) -> Result<(), DispatchError> {
        (**self).disable()
    }

    fn disable_for(&mut self, profile_id: Option<&str>) -> Result<(), DispatchError> {
        (**self).disable_for(profile_id)
    }

    fn reload(&mut self) -> Result<(), DispatchError> {
        (**self).reload()
    }

    fn reload_for(&mut self, profile_id: Option<&str>) -> Result<(), DispatchError> {
        (**self).reload_for(profile_id)
    }

    fn set_manual_location(&mut self, params: &RequestParams) -> Result<(), DispatchError> {
        (**self).set_manual_location(params)
    }

    fn clear_manual_location(&mut self) -> Result<(), DispatchError> {
        (**self).clear_manual_location()
    }

    fn clear_manual_location_for(&mut self, profile_id: Option<&str>) -> Result<(), DispatchError> {
        (**self).clear_manual_location_for(profile_id)
    }

    fn search_location(&mut self, query: &str) -> Result<Value, DispatchError> {
        (**self).search_location(query)
    }

    fn search_location_for(
        &mut self,
        query: &str,
        profile_id: Option<&str>,
    ) -> Result<Value, DispatchError> {
        (**self).search_location_for(query, profile_id)
    }

    fn refresh_periodic(&mut self) {
        (**self).refresh_periodic();
    }

    fn refresh_evidence(&mut self) -> Result<(), DispatchError> {
        (**self).refresh_evidence()
    }

    fn refresh_evidence_for(&mut self, profile_id: Option<&str>) -> Result<(), DispatchError> {
        (**self).refresh_evidence_for(profile_id)
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
    let profile_id = request.params().profile_id.as_deref();
    let empty =
        || encode_result_response(request_id, &serde_json::Value::Object(Default::default()));
    match request.method() {
        ApiMethod::StatusGet => match service.status_for(profile_id) {
            Ok(value) => encode_result_response(request_id, &value),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::ControlEnable => match service.enable_for(profile_id) {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::ControlDisable => match service.disable_for(profile_id) {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::ControlReload => match service.reload_for(profile_id) {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::GeoSet => match service.set_manual_location(request.params()) {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::GeoClear => match service.clear_manual_location_for(profile_id) {
            Ok(()) => empty(),
            Err(error) => encode_dispatch_error(request_id, error),
        },
        ApiMethod::GeoSearch => match request.params().query.as_deref() {
            Some(query) if !query.trim().is_empty() => {
                match service.search_location_for(query.trim(), profile_id) {
                    Ok(value) => encode_result_response(request_id, &value),
                    Err(error) => encode_dispatch_error(request_id, error),
                }
            }
            _ => encode_dispatch_error(request_id, DispatchError::InvalidLocation),
        },
        ApiMethod::Refresh => match service.refresh_evidence_for(profile_id) {
            Ok(()) => empty(),
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
