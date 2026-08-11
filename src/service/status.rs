//! Coordinate-free status snapshots for the local control API.
//!
//! The default snapshot deliberately cannot contain device addresses, node
//! credentials, provider payloads, private keys, or precise coordinates.

use serde::Serialize;

use super::api::SERVICE_API_ID;
use super::state::{ResponseMode, ServicePhase, ServiceState};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DesiredState {
    Disabled,
    Enabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineHealth {
    Stopped,
    Healthy,
    Unhealthy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StatusInputs {
    pub generation: u64,
    pub observed_at_unix: u64,
    pub desired_state: DesiredState,
    pub service_state: ServiceState,
    pub engine_health: EngineHealth,
    pub engine_uptime_seconds: u64,
    pub assigned_device_configured: bool,
}

#[derive(Serialize)]
struct StatusSnapshot {
    api_version: &'static str,
    generation: u64,
    observed_at: u64,
    desired_state: DesiredState,
    service_phase: &'static str,
    safety: SafetySnapshot,
    engine: EngineSnapshot,
    exit: ExitSnapshot,
    geo: GeoSnapshot,
    assigned_device_configured: bool,
    last_error: Option<StatusError>,
}

#[derive(Serialize)]
struct SafetySnapshot {
    redirect_present: bool,
    watchdog_armed: bool,
    scope_valid: bool,
    ipv6_ready: bool,
    response_mode: &'static str,
}

#[derive(Serialize)]
struct EngineSnapshot {
    health: EngineHealth,
    uptime_seconds: u64,
}

#[derive(Serialize)]
struct ExitSnapshot {
    state: &'static str,
    checked_at: Option<u64>,
}

#[derive(Serialize)]
struct GeoSnapshot {
    state: &'static str,
    expires_at: Option<u64>,
}

#[derive(Serialize)]
struct StatusError {
    component: &'static str,
    code: &'static str,
    at: u64,
    retryable: bool,
}

pub fn encode_status(inputs: &StatusInputs) -> Result<Vec<u8>, serde_json::Error> {
    let safety = inputs.service_state.safety();
    let snapshot = StatusSnapshot {
        api_version: SERVICE_API_ID,
        generation: inputs.generation,
        observed_at: inputs.observed_at_unix,
        desired_state: inputs.desired_state,
        service_phase: phase_name(inputs.service_state.phase()),
        safety: SafetySnapshot {
            redirect_present: safety.redirect_present(),
            watchdog_armed: safety.watchdog_armed(),
            scope_valid: safety.scope_valid(),
            ipv6_ready: safety.ipv6_ready(),
            response_mode: response_mode_name(inputs.service_state.response_mode()),
        },
        engine: EngineSnapshot {
            health: inputs.engine_health,
            uptime_seconds: inputs.engine_uptime_seconds,
        },
        exit: ExitSnapshot {
            state: "unknown",
            checked_at: None,
        },
        geo: GeoSnapshot {
            state: "unavailable",
            expires_at: None,
        },
        assigned_device_configured: inputs.assigned_device_configured,
        last_error: None,
    };
    serde_json::to_vec(&snapshot)
}

const fn phase_name(phase: ServicePhase) -> &'static str {
    match phase {
        ServicePhase::Disabled => "disabled",
        ServicePhase::Starting => "starting",
        ServicePhase::ReadyPassThrough => "ready_passthrough",
        ServicePhase::Intercepting => "intercepting",
        ServicePhase::DegradedPassThrough => "degraded_passthrough",
        ServicePhase::Draining => "draining",
    }
}

const fn response_mode_name(mode: ResponseMode) -> &'static str {
    match mode {
        ResponseMode::ForwardOriginal => "forward_original",
        ResponseMode::PatchAuthorized => "patch_authorized",
    }
}
