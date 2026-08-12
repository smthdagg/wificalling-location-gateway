//! Production service composition.
//!
//! [`WlocService`] ties the state machine, transactional runtime control, exit
//! probing, and Geo resolution into a working [`ServiceDispatch`]. Status
//! refreshes evidence through the bounded observation cache; enable and
//! disable drive the runtime through the transactional control path and keep
//! the state machine in sync. No WLOC response patching, CA, or interception
//! is implemented here.

use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::exitprobe::runtime::{observe_exit, ExitProbeRuntime};
use crate::exitprobe::{NodeRef, ProbeLimits};
use crate::georesolver::runtime::{resolve_geo, GeoProviderRuntime};
use crate::georesolver::{GeoResolution, ProviderRef};
use crate::service::control::{
    disable as control_disable, enable as control_enable, ControlError, RuntimeControl,
};
use crate::service::dispatch::{DispatchError, ServiceDispatch};
use crate::service::state::{reduce, ServiceEvent, ServicePhase, ServiceState};
use crate::service::status::{
    encode_status, DesiredState, EngineHealth, ExitState, GeoState, StatusInputs,
};
use crate::wloc::PatchTarget;

fn current_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn map_control_error(error: ControlError) -> DispatchError {
    match error {
        ControlError::InvalidSafetyScope => DispatchError::InvalidConfig,
        ControlError::EngineUnhealthy => DispatchError::EngineUnhealthy,
        ControlError::RedirectStillPresent => DispatchError::RedirectPresent,
        ControlError::CleanupUnsafe => DispatchError::CleanupUnsafe,
        ControlError::RuntimeFailure(_) => DispatchError::RuntimeFailure,
    }
}

/// Evidence state for the exit observation, used to derive the snapshot.
enum ExitEvidence {
    /// Never probed yet.
    None,
    /// Last probe succeeded and is still within the observation age.
    Verified(crate::exitprobe::ExitObservation),
    /// Last probe attempt failed; the previous observation was withdrawn so
    /// an unverified location can never be used.
    Unavailable,
}

/// Static configuration for the composed service.
pub struct WlocServiceConfig {
    pub node_ref: NodeRef,
    pub providers: Vec<ProviderRef>,
    pub probe_limits: ProbeLimits,
    pub scope_valid: bool,
    pub ipv6_ready: bool,
    pub assigned_device_configured: bool,
}

/// Production composition of the control API, runtime adapters, exit probe,
/// and Geo resolver.
pub struct WlocService<R: RuntimeControl, P: ExitProbeRuntime, G: GeoProviderRuntime> {
    state: ServiceState,
    runtime: R,
    probe: P,
    geo: G,
    node_ref: NodeRef,
    providers: Vec<ProviderRef>,
    probe_limits: ProbeLimits,
    scope_valid: bool,
    ipv6_ready: bool,
    assigned_device_configured: bool,
    desired_state: DesiredState,
    generation: u64,
    exit_evidence: ExitEvidence,
    geo_resolution: GeoResolution,
    /// Shared proxy patch target, updated whenever fresh Geo evidence lands.
    patch_sink: Option<Arc<Mutex<Option<PatchTarget>>>>,
}

impl<R: RuntimeControl, P: ExitProbeRuntime, G: GeoProviderRuntime> WlocService<R, P, G> {
    pub fn new(runtime: R, probe: P, geo: G, config: WlocServiceConfig) -> Self {
        Self {
            state: ServiceState::disabled(),
            runtime,
            probe,
            geo,
            node_ref: config.node_ref,
            providers: config.providers,
            probe_limits: config.probe_limits,
            scope_valid: config.scope_valid,
            ipv6_ready: config.ipv6_ready,
            assigned_device_configured: config.assigned_device_configured,
            desired_state: DesiredState::Disabled,
            generation: 0,
            exit_evidence: ExitEvidence::None,
            geo_resolution: GeoResolution::Unavailable,
            patch_sink: None,
        }
    }

    /// Attach a shared sink that receives the freshest `PatchTarget` whenever
    /// the service refreshes its Geo evidence. The MITM proxy reads this to
    /// decide the coordinates for `/clls/wloc` responses.
    pub fn with_patch_sink(mut self, sink: Arc<Mutex<Option<PatchTarget>>>) -> Self {
        self.patch_sink = Some(sink);
        self
    }

    /// Probe and resolve evidence when the cached observation is missing,
    /// stale, or the last probe failed.
    fn refresh_evidence_at(&mut self, now_unix: u64) {
        let fresh_enough = matches!(
            &self.exit_evidence,
            ExitEvidence::Verified(observation)
                if now_unix.saturating_sub(observation.checked_at_unix())
                    <= self.probe_limits.max_observation_age.as_secs()
        );
        if fresh_enough {
            return;
        }

        match observe_exit(
            &mut self.probe,
            self.node_ref.clone(),
            now_unix,
            self.probe_limits,
        ) {
            Ok(observation) => {
                self.exit_evidence = ExitEvidence::Verified(observation);
                let exit_ip = self
                    .last_exit_ip()
                    .expect("fresh observation always carries an exit IP");
                self.geo_resolution =
                    resolve_geo(&mut self.geo, exit_ip, &self.providers, now_unix);
                self.publish_patch_target();
            }
            Err(_) => {
                self.exit_evidence = ExitEvidence::Unavailable;
                self.geo_resolution = GeoResolution::Unavailable;
            }
        }
    }

    /// Publish the freshest Geo record as the proxy patch target, if a sink
    /// was attached and a Fresh record is available.
    fn publish_patch_target(&self) {
        let Some(sink) = &self.patch_sink else {
            return;
        };
        let GeoResolution::Fresh(record) = &self.geo_resolution else {
            return;
        };
        if let Ok(mut guard) = sink.lock() {
            *guard = Some(PatchTarget::new(record.latitude, record.longitude));
        }
    }

    fn last_exit_ip(&self) -> Option<IpAddr> {
        match &self.exit_evidence {
            ExitEvidence::Verified(observation) => Some(observation.exit_ip()),
            _ => None,
        }
    }

    /// Build the status snapshot inputs from the current evidence at `now_unix`.
    pub fn status_inputs_at(&self, now_unix: u64) -> StatusInputs {
        let (exit_state, exit_checked_at) = match &self.exit_evidence {
            ExitEvidence::Verified(observation) => {
                (ExitState::Verified, Some(observation.checked_at_unix()))
            }
            ExitEvidence::Unavailable => (ExitState::Unavailable, None),
            ExitEvidence::None => (ExitState::Unknown, None),
        };
        let (geo_state, geo_expires_at) = match &self.geo_resolution {
            GeoResolution::Fresh(record) => (GeoState::Fresh, Some(record.expires_at_unix)),
            GeoResolution::Uncertain => (GeoState::Uncertain, None),
            GeoResolution::Unavailable => (GeoState::Unavailable, None),
        };
        let (engine_health, engine_uptime) = match self.state.phase() {
            ServicePhase::Intercepting => (EngineHealth::Healthy, 0),
            _ => (EngineHealth::Stopped, 0),
        };

        StatusInputs {
            generation: self.generation,
            observed_at_unix: now_unix,
            desired_state: self.desired_state,
            service_state: self.state,
            engine_health,
            engine_uptime_seconds: engine_uptime,
            assigned_device_configured: self.assigned_device_configured,
            exit_state,
            exit_checked_at,
            geo_state,
            geo_expires_at,
        }
    }

    /// Serve a status snapshot at an explicit time (testable, deterministic).
    pub fn status_at(&mut self, now_unix: u64) -> Result<Value, DispatchError> {
        self.refresh_evidence_at(now_unix);
        let inputs = self.status_inputs_at(now_unix);
        let bytes = encode_status(&inputs).map_err(|_| DispatchError::Unavailable)?;
        serde_json::from_slice(&bytes).map_err(|_| DispatchError::Unavailable)
    }

    fn apply_enable_events(&mut self) {
        let events = [
            ServiceEvent::BeginEnable {
                scope_valid: self.scope_valid,
                ipv6_ready: self.ipv6_ready,
            },
            ServiceEvent::EngineReady,
            ServiceEvent::WatchdogArmed,
            ServiceEvent::RedirectInstalled,
        ];
        for event in events {
            match reduce(&self.state, event) {
                Ok(next) => self.state = next,
                Err(_) => break,
            }
        }
    }
}

impl<R: RuntimeControl, P: ExitProbeRuntime, G: GeoProviderRuntime> ServiceDispatch
    for WlocService<R, P, G>
{
    fn status(&mut self) -> Result<Value, DispatchError> {
        self.status_at(current_unix())
    }

    fn enable(&mut self) -> Result<(), DispatchError> {
        if self.state.phase() != ServicePhase::Disabled {
            return Err(DispatchError::InvalidConfig);
        }
        control_enable(&mut self.runtime, self.scope_valid, self.ipv6_ready)
            .map_err(map_control_error)?;
        self.apply_enable_events();
        self.desired_state = DesiredState::Enabled;
        Ok(())
    }

    fn disable(&mut self) -> Result<(), DispatchError> {
        if self.state.phase() == ServicePhase::Disabled {
            return Ok(());
        }
        control_disable(&mut self.runtime).map_err(map_control_error)?;
        for event in [ServiceEvent::BeginDisable, ServiceEvent::EngineStopped] {
            match reduce(&self.state, event) {
                Ok(next) => self.state = next,
                Err(_) => break,
            }
        }
        self.desired_state = DesiredState::Disabled;
        Ok(())
    }

    fn reload(&mut self) -> Result<(), DispatchError> {
        // Configuration reload is a later concern; the redirect state is
        // intentionally untouched here.
        Ok(())
    }
}
