//! Production service composition.
//!
//! [`WlocService`] ties the state machine, transactional runtime control, exit
//! probing, and Geo resolution into a working [`ServiceDispatch`]. Enable and
//! explicit refresh drive evidence acquisition; periodic health reporting
//! rechecks only automatic location evidence, while manual location is local
//! state. No WLOC response patching, CA, or interception is implemented here.

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::exitprobe::runtime::{observe_exit, ExitProbeRuntime};
use crate::exitprobe::{NodeRef, ProbeLimits};
use crate::georesolver::runtime::{resolve_geo, GeoProviderRuntime};
use crate::georesolver::{GeoResolution, ProviderRef};
use crate::service::api::RequestParams;
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
    /// LAN IP of the device whose node binding the location follows
    /// (shown in the monitor page).
    pub assigned_device: Option<String>,
    /// Reverse-geocode endpoint used to fill in country/city/timezone for a
    /// manual coordinate switch. `None` disables the background lookup
    /// (tests). Production uses the public Nominatim TLS endpoint.
    pub reverse_geo_lookup: Option<(String, u16)>,
}

/// One reverse-geocode lookup in flight: the generation it was started for
/// and the outcome once the worker thread lands (`None` while running).
struct ManualGeoLookup {
    generation: u64,
    outcome: Option<Result<crate::georesolver::geocode::ReverseGeoResult, ()>>,
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
    /// Location source: follow the node exit (`Auto`) or a manual preset.
    geo_source: GeoSource,
    /// Reverse-geocoded place info for the manual preset (country/city/
    /// timezone), so the status file can show them without an auto probe.
    manual_geo: Option<crate::georesolver::geocode::ReverseGeoResult>,
    /// Bumped on every manual coordinate change; background lookups carry the
    /// generation they were started for and stale outcomes are dropped.
    geo_generation: u64,
    /// In-flight reverse-geocode lookup for the current manual target. The
    /// worker thread (never the control path) writes the outcome here; the
    /// next status/refresh cycle applies it.
    manual_geo_pending: std::sync::Arc<std::sync::Mutex<Option<ManualGeoLookup>>>,
    /// Fingerprint of the probe configuration (Gateway sing-box.json plus
    /// the device-policy UCI file) at the last successful probe. A change
    /// means the followed device's node was switched; fresh evidence is
    /// then re-probed immediately.
    last_probe_fingerprint: Option<u64>,
    /// Last probe failure reason (shown in the monitor when the exit IP is
    /// unknown); cleared on a successful probe.
    last_probe_error: Option<String>,
    /// Reverse-geocode endpoint for manual place-info lookups (production:
    /// Nominatim TLS; tests: mock port or `None` to disable).
    reverse_geo_lookup: Option<(String, u16)>,
    /// LAN IP of the device whose node binding the location follows.
    assigned_device: Option<String>,
    /// Root-local status JSON written on every status snapshot (includes GPS
    /// for the LuCI admin UI; never exposed through the control API).
    status_file: Option<PathBuf>,
    /// Append-only usage log (target updates and rewrites).
    events_file: Option<PathBuf>,
}

/// How the proxy patch target is chosen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeoSource {
    /// Follow the node exit: resolve Geo for the observed exit IP.
    Auto,
    /// Use a fixed manual preset.
    Manual { latitude: f64, longitude: f64 },
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
            assigned_device: config.assigned_device,
            desired_state: DesiredState::Disabled,
            generation: 0,
            exit_evidence: ExitEvidence::None,
            geo_resolution: GeoResolution::Unavailable,
            patch_sink: None,
            geo_source: GeoSource::Auto,
            manual_geo: None,
            geo_generation: 0,
            manual_geo_pending: std::sync::Arc::new(std::sync::Mutex::new(None)),
            reverse_geo_lookup: config.reverse_geo_lookup,
            last_probe_fingerprint: None,
            last_probe_error: None,
            status_file: None,
            events_file: None,
        }
    }

    /// Attach a shared sink that receives the freshest `PatchTarget` whenever
    /// the service refreshes its Geo evidence. The MITM proxy reads this to
    /// decide the coordinates for `/clls/wloc` responses.
    pub fn with_patch_sink(mut self, sink: Arc<Mutex<Option<PatchTarget>>>) -> Self {
        self.patch_sink = Some(sink);
        self
    }

    /// Persist a root-local status JSON (with GPS for the admin UI) after
    /// every status snapshot, and append usage events to `events_file`.
    pub fn with_state_files(mut self, status_file: PathBuf, events_file: PathBuf) -> Self {
        self.status_file = Some(status_file);
        self.events_file = Some(events_file);
        self
    }

    /// Probe and resolve evidence when the cached observation is missing,
    /// stale, or the last probe failed.
    fn refresh_evidence_at(&mut self, now_unix: u64) -> bool {
        // Manual mode pins the location to the preset coordinates; exit
        // probing exists only to drive auto-follow, so it is skipped
        // entirely in manual mode (no reverse probe, no misleading IP).
        if matches!(self.geo_source, GeoSource::Manual { .. }) {
            return true;
        }
        let fingerprint = self.probe.config_fingerprint();
        let fresh = matches!(
            &self.exit_evidence,
            ExitEvidence::Verified(observation)
                if now_unix.saturating_sub(observation.checked_at_unix())
                    <= self.probe_limits.max_observation_age.as_secs()
        );
        if !probe_needed(fresh, fingerprint, self.last_probe_fingerprint) {
            return matches!(self.geo_resolution, GeoResolution::Fresh(_));
        }

        match observe_exit(
            &mut self.probe,
            self.node_ref.clone(),
            now_unix,
            self.probe_limits,
        ) {
            Ok(observation) => {
                let previous_exit_ip = self.last_exit_ip();
                self.exit_evidence = ExitEvidence::Verified(observation);
                self.last_probe_fingerprint = fingerprint;
                self.last_probe_error = None;
                let exit_ip = self
                    .last_exit_ip()
                    .expect("fresh observation always carries an exit IP");
                if previous_exit_ip != Some(exit_ip) {
                    self.advance_generation();
                }
                let geo_refresh_needed = previous_exit_ip != Some(exit_ip)
                    || !matches!(
                        &self.geo_resolution,
                        GeoResolution::Fresh(record) if record.expires_at_unix > now_unix
                    );
                if geo_refresh_needed {
                    eprintln!(
                        "wloc refresh: probing exit {exit_ip} with {} provider(s)",
                        self.providers.len()
                    );
                    self.geo_resolution =
                        resolve_geo(&mut self.geo, exit_ip, &self.providers, now_unix);
                    eprintln!("wloc refresh: geo result {:?}", self.geo_resolution);
                }
                self.publish_patch_target();
                matches!(self.geo_resolution, GeoResolution::Fresh(_))
            }
            Err(error) => {
                self.exit_evidence = ExitEvidence::Unavailable;
                self.geo_resolution = GeoResolution::Unavailable;
                self.last_probe_error = Some(error.to_string());
                self.publish_patch_target();
                false
            }
        }
    }

    /// Publish the current patch target: a manual preset when set, otherwise
    /// the freshest Geo record (in Auto mode).
    fn publish_patch_target(&self) {
        let target = match self.geo_source {
            GeoSource::Manual {
                latitude,
                longitude,
            } => Some(PatchTarget::new(latitude, longitude)),
            GeoSource::Auto => match &self.geo_resolution {
                GeoResolution::Fresh(record) => {
                    Some(PatchTarget::new(record.latitude, record.longitude))
                }
                _ => None,
            },
        };
        let changed = match &self.patch_sink {
            Some(sink) => match sink.lock() {
                Ok(mut guard) => {
                    let changed = *guard != target;
                    if changed {
                        *guard = target;
                    }
                    changed
                }
                Err(_) => true,
            },
            None => true,
        };
        if changed {
            self.append_target_event(target);
        }
    }

    fn advance_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Append a target-change event to the usage log.
    fn append_target_event(&self, target: Option<PatchTarget>) {
        let Some(events_file) = &self.events_file else {
            return;
        };
        let (source, country, city) = match self.geo_source {
            GeoSource::Manual { .. } => ("manual", None::<&str>, None::<&str>),
            GeoSource::Auto => (
                "auto",
                match &self.geo_resolution {
                    GeoResolution::Fresh(record) => Some(record.country_code.as_str()),
                    _ => None,
                },
                match &self.geo_resolution {
                    GeoResolution::Fresh(record) => Some(record.city.as_str()),
                    _ => None,
                },
            ),
        };
        let event = serde_json::json!({
            "type": "target_updated",
            "time": current_unix(),
            "source": source,
            "country_code": country,
            "city": city,
            "latitude": target.map(|t| t.latitude),
            "longitude": target.map(|t| t.longitude),
        });
        crate::service::append_event_line(events_file, &event);
    }

    /// Write the root-local status JSON (includes GPS for the admin UI).
    pub fn write_status_file(&self, inputs: &StatusInputs) {
        let Some(status_file) = &self.status_file else {
            return;
        };
        // The displayed location is the effective patch target: the manual
        // preset when one is set, otherwise the fresh Geo observation.
        let (latitude, longitude, country_code, city, timezone) = match &self.geo_source {
            GeoSource::Manual {
                latitude,
                longitude,
            } => {
                let manual = self.manual_geo.as_ref();
                (
                    Some(*latitude),
                    Some(*longitude),
                    manual.map(|m| m.country_code.clone()),
                    manual.map(|m| m.city.clone()),
                    manual.map(|m| m.timezone.clone()),
                )
            }
            GeoSource::Auto => match &self.geo_resolution {
                GeoResolution::Fresh(record) => (
                    Some(record.latitude),
                    Some(record.longitude),
                    Some(record.country_code.clone()),
                    Some(record.city.clone()),
                    Some(record.timezone.clone()),
                ),
                _ => (None, None, None, None, None),
            },
        };
        // In manual mode the exit IP is not meaningful (no probing runs);
        // only auto-follow reports the observed exit.
        let exit_ip = match self.geo_source {
            GeoSource::Manual { .. } => None,
            GeoSource::Auto => self.last_exit_ip().map(|ip| ip.to_string()),
        };
        let status = serde_json::json!({
            "generation": inputs.generation,
            "observed_at": inputs.observed_at_unix,
            "service_phase": match inputs.service_state.phase() {
                ServicePhase::Disabled => "disabled",
                ServicePhase::Starting => "starting",
                ServicePhase::ReadyPassThrough => "ready_passthrough",
                ServicePhase::Intercepting => "intercepting",
                ServicePhase::DegradedPassThrough => "degraded_passthrough",
                ServicePhase::Draining => "draining",
            },
            "geo_source": match self.geo_source {
                GeoSource::Auto => "auto",
                GeoSource::Manual { .. } => "manual",
            },
            "assigned_device": self.assigned_device.clone(),
            "desired_state": serde_json::to_value(inputs.desired_state).ok(),
            "exit": {
                "state": serde_json::to_value(inputs.exit_state).ok(),
                "ip": exit_ip,
                "checked_at": inputs.exit_checked_at,
                "last_error": self.last_probe_error,
            },
            "geo": {
                "state": serde_json::to_value(inputs.geo_state).ok(),
                "country_code": country_code,
                "city": city,
                "latitude": latitude,
                "longitude": longitude,
                "timezone": timezone,
                "expires_at": inputs.geo_expires_at,
            },
            "engine": {
                "health": serde_json::to_value(inputs.engine_health).ok(),
            },
            "assigned_device_configured": inputs.assigned_device_configured,
        });
        if let Some(parent) = status_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(&status) {
            let _ = std::fs::write(status_file, text);
        }
    }

    /// The current location source, for the status snapshot.
    pub fn geo_source(&self) -> GeoSource {
        self.geo_source
    }

    /// Apply a manual location preset: a place query is geocoded online, or an
    /// explicit WGS84 coordinate pair is accepted directly.
    pub fn set_manual_location(
        &mut self,
        params: &crate::service::api::RequestParams,
    ) -> Result<(), crate::service::dispatch::DispatchError> {
        let (latitude, longitude) = match (&params.query, params.latitude, params.longitude) {
            (Some(query), None, None) if !query.trim().is_empty() => {
                crate::georesolver::geocode::geocode(query.trim())
                    .map_err(|_| crate::service::dispatch::DispatchError::InvalidLocation)?
            }
            (None, Some(latitude), Some(longitude)) => (latitude, longitude),
            _ => return Err(crate::service::dispatch::DispatchError::InvalidLocation),
        };
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            return Err(crate::service::dispatch::DispatchError::InvalidLocation);
        }
        self.geo_source = GeoSource::Manual {
            latitude,
            longitude,
        };
        // A coordinate mode switch is a local control operation and must not
        // block the root-only control socket on an external reverse-geocode
        // request. The coordinates are authoritative; optional place metadata
        // is cleared and refilled by a background lookup with a strict
        // timeout (never on this path).
        self.manual_geo = None;
        self.geo_generation += 1;
        self.advance_generation();
        self.spawn_manual_geo_lookup(latitude, longitude);
        self.publish_patch_target();
        self.refresh_state_file();
        Ok(())
    }

    /// Start a background reverse-geocode lookup for the current manual
    /// coordinates. The worker thread does all network I/O with a strict
    /// timeout and writes only its own generation, so a late result can
    /// never overwrite a newer target.
    fn spawn_manual_geo_lookup(&self, latitude: f64, longitude: f64) {
        let generation = self.geo_generation;
        let slot = std::sync::Arc::clone(&self.manual_geo_pending);
        *slot.lock().expect("manual geo slot lock") = Some(ManualGeoLookup {
            generation,
            outcome: None,
        });
        let Some((host, port)) = &self.reverse_geo_lookup else {
            return;
        };
        let host = host.clone();
        let port = *port;
        std::thread::spawn(move || {
            let outcome =
                crate::georesolver::geocode::reverse_geocode_at(&host, port, latitude, longitude)
                    .map_err(|_| ());
            if let Ok(mut slot) = slot.lock() {
                if let Some(pending) = slot.as_mut() {
                    if pending.generation == generation {
                        pending.outcome = Some(outcome);
                    }
                }
            }
        });
    }

    /// Apply a completed background lookup if it still matches the current
    /// manual target, then refresh the status file. Called from the status
    /// and periodic-refresh paths only - never from the control path.
    fn consume_pending_manual_geo(&mut self) {
        let completed = match self.manual_geo_pending.lock() {
            Ok(mut slot) => match slot.as_mut() {
                Some(pending) => pending
                    .outcome
                    .take()
                    .map(|outcome| (pending.generation, outcome)),
                None => None,
            },
            Err(_) => None,
        };
        if let Some((generation, outcome)) = completed {
            if generation == self.geo_generation {
                self.manual_geo = outcome.ok();
                self.refresh_state_file();
            }
        }
    }

    /// Return to automatic node-following location.
    pub fn clear_manual_location(&mut self) -> Result<(), crate::service::dispatch::DispatchError> {
        if !matches!(self.geo_source, GeoSource::Manual { .. }) {
            return Ok(());
        }
        self.geo_source = GeoSource::Auto;
        self.manual_geo = None;
        self.geo_generation += 1;
        self.advance_generation();
        if let Ok(mut slot) = self.manual_geo_pending.lock() {
            *slot = None;
        }
        self.force_evidence_refresh();
        Ok(())
    }

    /// Force an immediate re-probe of the followed node, discarding cached
    /// evidence. Used by the monitor's manual refresh: the probe runs
    /// inline (bounded by the probe timeout), then the status file is
    /// rewritten so the UI shows the new exit IP without waiting for the
    /// periodic housekeeping tick.
    pub fn force_evidence_refresh(&mut self) {
        self.exit_evidence = ExitEvidence::None;
        self.last_probe_fingerprint = None;
        let _ = self.refresh_evidence_at(current_unix());
        self.refresh_state_file();
    }

    /// Test hook for a deterministic periodic health tick.
    #[doc(hidden)]
    pub fn refresh_periodic_at(&mut self, now_unix: u64) {
        self.consume_pending_manual_geo();
        if !self.refresh_runtime_health() {
            self.refresh_state_file_at(now_unix);
            return;
        }
        let _ = self.refresh_evidence_at(now_unix);
        if self.desired_state == DesiredState::Enabled
            && self.state.phase() == ServicePhase::Disabled
        {
            let _ = self.enable();
        }
        self.refresh_state_file_at(now_unix);
    }

    /// Withdraw interception when the shared Gateway engine disappears. Keep
    /// the desired state enabled so the next healthy periodic tick can restore
    /// it after procd restarts sing-box.
    fn refresh_runtime_health(&mut self) -> bool {
        if !matches!(
            self.state.phase(),
            ServicePhase::Starting | ServicePhase::ReadyPassThrough | ServicePhase::Intercepting
        ) {
            return true;
        }
        let healthy = self.runtime.engine_healthy().unwrap_or(false);
        if healthy {
            return true;
        }
        eprintln!("wloc-service: shared Gateway engine is unhealthy; withdrawing interception");
        if control_disable(&mut self.runtime).is_ok() {
            for event in [ServiceEvent::BeginDisable, ServiceEvent::EngineStopped] {
                if let Ok(next) = reduce(&self.state, event) {
                    self.state = next;
                }
            }
        }
        false
    }

    /// The generation of the current manual target (test hook).
    #[doc(hidden)]
    pub fn manual_geo_generation(&self) -> u64 {
        self.geo_generation
    }

    /// Test hook: simulate the background worker writing its outcome for
    /// `generation`. Mirrors the worker's stale-guard: an outcome for an old
    /// generation is not stored.
    #[doc(hidden)]
    pub fn finish_manual_geo_lookup(
        &mut self,
        generation: u64,
        outcome: Result<crate::georesolver::geocode::ReverseGeoResult, ()>,
    ) {
        if let Ok(mut slot) = self.manual_geo_pending.lock() {
            if let Some(pending) = slot.as_mut() {
                if pending.generation == generation {
                    pending.outcome = Some(outcome);
                }
            }
        }
    }

    /// Refresh the root-local status file at a supplied time.
    fn refresh_state_file_at(&self, now_unix: u64) {
        let inputs = self.status_inputs_at(now_unix);
        self.write_status_file(&inputs);
    }

    /// Refresh the root-local status file immediately so the LuCI UI sees a
    /// control change without waiting for the next status poll.
    fn refresh_state_file(&self) {
        self.refresh_state_file_at(current_unix());
    }

    fn last_exit_ip(&self) -> Option<IpAddr> {
        match &self.exit_evidence {
            ExitEvidence::Verified(observation) => Some(observation.exit_ip()),
            _ => None,
        }
    }

    /// Build the status snapshot inputs from the current evidence at `now_unix`.
    pub fn status_inputs_at(&self, now_unix: u64) -> StatusInputs {
        let (exit_state, exit_checked_at) = match &self.geo_source {
            // Manual mode: exit probing is skipped by design; report the
            // healthy "manual" state instead of unknown/unavailable.
            GeoSource::Manual { .. } => (ExitState::Manual, None),
            GeoSource::Auto => match &self.exit_evidence {
                ExitEvidence::Verified(observation)
                    if now_unix.saturating_sub(observation.checked_at_unix())
                        > self.probe_limits.max_observation_age.as_secs() =>
                {
                    (ExitState::Stale, Some(observation.checked_at_unix()))
                }
                ExitEvidence::Verified(observation) => {
                    (ExitState::Verified, Some(observation.checked_at_unix()))
                }
                ExitEvidence::Unavailable => (ExitState::Unavailable, None),
                ExitEvidence::None => (ExitState::Unknown, None),
            },
        };
        let (geo_state, geo_expires_at) = match &self.geo_source {
            // Manual mode: the manual preset is the source of truth.
            GeoSource::Manual { .. } => (GeoState::Manual, None),
            GeoSource::Auto => match &self.geo_resolution {
                GeoResolution::Fresh(record) => (GeoState::Fresh, Some(record.expires_at_unix)),
                GeoResolution::Uncertain => (GeoState::Uncertain, None),
                GeoResolution::Unavailable => (GeoState::Unavailable, None),
            },
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
            geo_source: match self.geo_source {
                GeoSource::Auto => crate::service::status::GeoSourceState::Auto,
                GeoSource::Manual { .. } => crate::service::status::GeoSourceState::Manual,
            },
        }
    }

    /// Serve a status snapshot at an explicit time (testable, deterministic).
    pub fn status_at(&mut self, now_unix: u64) -> Result<Value, DispatchError> {
        let inputs = self.status_inputs_at(now_unix);
        self.write_status_file(&inputs);
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
        // The LuCI monitor polls status; applying a completed background
        // lookup here refills country/city/timezone without any network on
        // the control path.
        self.consume_pending_manual_geo();
        self.status_at(current_unix())
    }

    fn enable(&mut self) -> Result<(), DispatchError> {
        if self.state.phase() != ServicePhase::Disabled {
            return Err(DispatchError::InvalidConfig);
        }
        // A daemon restart starts with an in-memory Disabled state while a
        // previous process may have left a redirect behind. Always withdraw
        // that stale state before rejecting an unsafe IPv6/scope configuration.
        if !self.scope_valid || !self.ipv6_ready || !self.assigned_device_configured {
            control_disable(&mut self.runtime).map_err(map_control_error)?;
            return Err(DispatchError::InvalidConfig);
        }
        // Prime the device -> exit IP -> Geo association before the runtime
        // installs interception. Otherwise the first WLOC request after a
        // restart can pass through with an empty target while the first
        // periodic refresh is still running.
        if !self.refresh_evidence_at(current_unix()) {
            return Err(DispatchError::Unavailable);
        }
        control_enable(&mut self.runtime, self.scope_valid, self.ipv6_ready)
            .map_err(map_control_error)?;
        self.apply_enable_events();
        self.desired_state = DesiredState::Enabled;
        Ok(())
    }

    fn disable(&mut self) -> Result<(), DispatchError> {
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

    fn set_manual_location(&mut self, params: &RequestParams) -> Result<(), DispatchError> {
        self.set_manual_location(params)
    }

    fn clear_manual_location(&mut self) -> Result<(), DispatchError> {
        self.clear_manual_location()
    }

    fn search_location(&mut self, query: &str) -> Result<Value, DispatchError> {
        let result = crate::georesolver::geocode::geocode_with_name(query)
            .map_err(|_| DispatchError::InvalidLocation)?;
        Ok(serde_json::json!({
            "city": result.city,
            "latitude": result.latitude,
            "longitude": result.longitude,
        }))
    }

    fn refresh_periodic(&mut self) {
        self.refresh_periodic_at(current_unix());
    }

    fn refresh_evidence(&mut self) -> Result<(), DispatchError> {
        self.force_evidence_refresh();
        Ok(())
    }
}

/// Whether a fresh probe is required: missing/stale evidence, a previous
/// failure, or a changed probe configuration (the followed device's node
/// was switched in the Gateway settings).
fn probe_needed(
    fresh: bool,
    current_fingerprint: Option<u64>,
    last_fingerprint: Option<u64>,
) -> bool {
    !fresh || current_fingerprint != last_fingerprint
}

#[cfg(test)]
mod probe_needed_tests {
    use super::probe_needed;

    #[test]
    fn stale_or_missing_evidence_always_reprobes() {
        assert!(probe_needed(false, Some(1), Some(1)));
        assert!(probe_needed(false, None, None));
    }

    #[test]
    fn fresh_evidence_is_kept_when_config_is_unchanged() {
        assert!(!probe_needed(true, Some(1), Some(1)));
        // Probes without fingerprint support (None == None) never force a
        // re-probe on their own.
        assert!(!probe_needed(true, None, None));
    }

    #[test]
    fn node_switch_changes_the_fingerprint_and_forces_reprobe() {
        // The followed device's node changed in the Gateway settings:
        // even fresh evidence must be re-probed immediately.
        assert!(probe_needed(true, Some(2), Some(1)));
        // A probe that gained fingerprint support after startup also
        // re-probes once.
        assert!(probe_needed(true, Some(1), None));
    }
}
