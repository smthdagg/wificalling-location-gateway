//! Production service composition contract.
//!
//! WlocService ties the state machine, transactional runtime control, exit
//! probing, and Geo resolution into a working ServiceDispatch. Status refreshes
//! evidence through the bounded cache; enable/disable drive the runtime through
//! the transactional control path and keep the state machine in sync.

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use wificalling_location_gateway::app::{WlocService, WlocServiceConfig};
use wificalling_location_gateway::exitprobe::runtime::{ExitProbeRuntime, ProbeFailure};
use wificalling_location_gateway::exitprobe::{NodeRef, ProbeLimits};
use wificalling_location_gateway::georesolver::runtime::{GeoProviderRuntime, ProviderFailure};
use wificalling_location_gateway::georesolver::ProviderRef;
use wificalling_location_gateway::service::control::{RuntimeControl, RuntimeFailure};
use wificalling_location_gateway::service::dispatch::{DispatchError, ServiceDispatch};
use wificalling_location_gateway::service::GeoRecord;

const WAN_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
const EXIT_A: IpAddr = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
const EXIT_B: IpAddr = IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9));

fn limits() -> ProbeLimits {
    ProbeLimits {
        max_observation_age: Duration::from_secs(60),
    }
}

fn record(now_unix: u64, latitude: f64, longitude: f64) -> GeoRecord {
    GeoRecord {
        country_code: "US".to_owned(),
        latitude,
        longitude,
        timezone: "America/Los_Angeles".to_owned(),
        expires_at_unix: now_unix + 3_600,
    }
}

struct OkRuntime {
    healthy: bool,
    install_fails: bool,
}

impl RuntimeControl for OkRuntime {
    fn start_engine_passthrough(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn engine_healthy(&mut self) -> Result<bool, RuntimeFailure> {
        Ok(self.healthy)
    }
    fn arm_watchdog(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn install_exact_redirect(&mut self) -> Result<(), RuntimeFailure> {
        if self.install_fails {
            Err(RuntimeFailure)
        } else {
            Ok(())
        }
    }
    fn remove_redirect(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn redirect_present(&mut self) -> Result<bool, RuntimeFailure> {
        Ok(false)
    }
    fn disarm_watchdog(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn drain_engine(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn stop_engine(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
}

struct SequenceProbe {
    results: Vec<Result<IpAddr, ProbeFailure>>,
    index: usize,
}

impl ExitProbeRuntime for SequenceProbe {
    fn probe_exit_ip(&mut self) -> Result<IpAddr, ProbeFailure> {
        let result = self.results[self.index];
        self.index += 1;
        result
    }
    fn router_wan_ips(&mut self) -> Result<Vec<IpAddr>, ProbeFailure> {
        Ok(vec![WAN_V4])
    }
}

struct SequenceGeo {
    results: Vec<Result<Option<(IpAddr, GeoRecord)>, ProviderFailure>>,
    index: usize,
}

impl GeoProviderRuntime for SequenceGeo {
    fn lookup(
        &mut self,
        _provider: ProviderRef,
        _ip: IpAddr,
    ) -> Result<Option<(IpAddr, GeoRecord)>, ProviderFailure> {
        let result = self.results[self.index].clone();
        self.index += 1;
        result
    }
}

fn build(
    runtime: OkRuntime,
    probe: SequenceProbe,
    geo: SequenceGeo,
) -> WlocService<OkRuntime, SequenceProbe, SequenceGeo> {
    WlocService::new(
        runtime,
        probe,
        geo,
        WlocServiceConfig {
            node_ref: NodeRef::new("node-1").unwrap(),
            providers: vec![ProviderRef::new("geo-a").unwrap()],
            probe_limits: limits(),
            scope_valid: true,
            ipv6_ready: true,
            assigned_device_configured: true,
        },
    )
}

fn fresh_probe() -> SequenceProbe {
    SequenceProbe {
        results: vec![Ok(EXIT_A)],
        index: 0,
    }
}

fn fresh_geo(now_unix: u64) -> SequenceGeo {
    SequenceGeo {
        results: vec![Ok(Some((EXIT_A, record(now_unix, 37.77, -122.41))))],
        index: 0,
    }
}

#[test]
fn status_reports_verified_exit_and_fresh_geo() {
    let now = 1_000_000;
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    );

    let status = service.status_at(now).unwrap();
    assert_eq!(status["exit"]["state"], "verified");
    assert_eq!(status["geo"]["state"], "fresh");
    assert_eq!(status["service_phase"], "disabled");
}

#[test]
fn status_reports_unavailable_when_probe_fails() {
    let now = 1_000_000;
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        SequenceProbe {
            results: vec![Err(ProbeFailure::Timeout)],
            index: 0,
        },
        SequenceGeo {
            results: vec![],
            index: 0,
        },
    );

    let status = service.status_at(now).unwrap();
    assert_eq!(status["exit"]["state"], "unavailable");
    assert_eq!(status["geo"]["state"], "unavailable");
}

#[test]
fn status_reports_uncertain_when_geo_conflicts() {
    let now = 1_000_000;
    let mut service = WlocService::new(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        SequenceGeo {
            results: vec![
                Ok(Some((EXIT_A, record(now, 37.77, -122.41)))),
                Ok(Some((EXIT_A, record(now, 51.50, -0.12)))),
            ],
            index: 0,
        },
        WlocServiceConfig {
            node_ref: NodeRef::new("node-1").unwrap(),
            providers: vec![
                ProviderRef::new("geo-a").unwrap(),
                ProviderRef::new("geo-b").unwrap(),
            ],
            probe_limits: limits(),
            scope_valid: true,
            ipv6_ready: true,
            assigned_device_configured: true,
        },
    );

    let status = service.status_at(now).unwrap();
    assert_eq!(status["exit"]["state"], "verified");
    assert_eq!(status["geo"]["state"], "uncertain");
}

#[test]
fn fresh_evidence_is_cached_and_stale_evidence_refreshes() {
    let now = 1_000_000;
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        SequenceProbe {
            results: vec![Ok(EXIT_A), Ok(EXIT_B)],
            index: 0,
        },
        SequenceGeo {
            results: vec![
                Ok(Some((EXIT_A, record(now, 37.77, -122.41)))),
                Ok(Some((EXIT_B, record(now, 51.50, -0.12)))),
            ],
            index: 0,
        },
    );

    let first = service.status_at(now).unwrap();
    assert_eq!(first["exit"]["state"], "verified");

    // Within the observation age the evidence is cached: no re-probe.
    let cached = service.status_at(now + 30).unwrap();
    assert_eq!(cached["exit"]["state"], "verified");

    // Past the observation age the service re-probes and sees the new exit.
    let refreshed = service.status_at(now + 61).unwrap();
    assert_eq!(refreshed["exit"]["state"], "verified");
}

#[test]
fn enable_success_drives_state_to_intercepting() {
    let now = 1_000_000;
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    );

    service.enable().unwrap();
    let status = service.status_at(now).unwrap();
    assert_eq!(status["service_phase"], "intercepting");
    assert_eq!(status["desired_state"], "enabled");
}

#[test]
fn enable_with_unhealthy_engine_returns_engine_unhealthy() {
    let now = 1_000_000;
    let mut service = build(
        OkRuntime {
            healthy: false,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    );

    assert_eq!(service.enable(), Err(DispatchError::EngineUnhealthy));
    let status = service.status_at(now).unwrap();
    assert_eq!(status["service_phase"], "disabled");
}

#[test]
fn enable_install_failure_compensates_and_reports_cleanup_safe() {
    let now = 1_000_000;
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: true,
        },
        fresh_probe(),
        fresh_geo(now),
    );

    // The transactional control withdraws the redirect and stops the engine;
    // the state machine must not advance past the failure.
    assert_eq!(service.enable(), Err(DispatchError::RuntimeFailure));
    let status = service.status_at(now).unwrap();
    assert_eq!(status["service_phase"], "disabled");
}

#[test]
fn disable_returns_to_disabled_and_is_idempotent() {
    let now = 1_000_000;
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    );

    service.enable().unwrap();
    service.disable().unwrap();
    let status = service.status_at(now).unwrap();
    assert_eq!(status["service_phase"], "disabled");
    assert_eq!(status["desired_state"], "disabled");

    // Disabling again is idempotent.
    assert!(service.disable().is_ok());
}

#[test]
fn status_never_exposes_coordinates_or_device_material() {
    let now = 1_000_000;
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    );

    let status = service.status_at(now).unwrap();
    let text = status.to_string();
    for forbidden in ["latitude", "longitude", "device_ip", "node-1", "credential"] {
        assert!(!text.contains(forbidden), "leaked field: {forbidden}");
    }
}
