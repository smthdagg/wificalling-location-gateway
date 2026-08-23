//! Production service composition contract.
//!
//! WlocService ties the state machine, transactional runtime control, exit
//! probing, and Geo resolution into a working ServiceDispatch. Status refreshes
//! evidence through the bounded cache; enable/disable drive the runtime through
//! the transactional control path and keep the state machine in sync.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wificalling_location_gateway::app::{WlocService, WlocServiceConfig};
use wificalling_location_gateway::exitprobe::runtime::{ExitProbeRuntime, ProbeFailure};
use wificalling_location_gateway::exitprobe::{NodeRef, ProbeLimits};
use wificalling_location_gateway::georesolver::geocode::ReverseGeoResult;
use wificalling_location_gateway::georesolver::runtime::{GeoProviderRuntime, ProviderFailure};
use wificalling_location_gateway::georesolver::ProviderRef;
use wificalling_location_gateway::service::api::RequestParams;
use wificalling_location_gateway::service::control::{RuntimeControl, RuntimeFailure};
use wificalling_location_gateway::service::dispatch::{DispatchError, ServiceDispatch};
use wificalling_location_gateway::service::GeoRecord;

const WAN_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
const EXIT_A: IpAddr = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
const EXIT_B: IpAddr = IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9));

#[test]
fn coordinate_mode_switch_is_local_only_and_never_waits_for_reverse_geocoding() {
    let source = include_str!("../src/app.rs");
    let block = source
        .split("pub fn set_manual_location")
        .nth(1)
        .expect("manual location implementation")
        .split("pub fn clear_manual_location")
        .next()
        .expect("manual location function boundary");
    assert!(
        !block.contains("reverse_geocode("),
        "a fixed-coordinate mode switch must update locally without blocking the control socket"
    );
}

#[test]
fn dispatch_geo_set_and_clear_drive_the_real_service() {
    // The RPC bridge routes geo.set/geo.clear through the real dispatch
    // implementation: coordinates publish a manual target, clearing returns
    // to automatic node-following (regression for the mode-switch fix).
    use serde_json::json;
    use wificalling_location_gateway::service::api::{decode_request, SERVICE_API_ID};
    use wificalling_location_gateway::service::dispatch::dispatch;

    let now = 1_000_000;
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    );

    let set = decode_request(
        &serde_json::to_vec(&json!({
            "api_version": SERVICE_API_ID,
            "request_id": "req-set",
            "method": "geo.set",
            "params": { "latitude": 22.3193, "longitude": 114.1694 }
        }))
        .unwrap(),
    )
    .unwrap();
    let response = dispatch(&set, &mut service).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(parsed["result"], json!({}));
    assert_eq!(service.status_at(now).unwrap()["geo_source"], "manual");

    let clear = decode_request(
        &serde_json::to_vec(&json!({
            "api_version": SERVICE_API_ID,
            "request_id": "req-clear",
            "method": "geo.clear",
            "params": {}
        }))
        .unwrap(),
    )
    .unwrap();
    let response = dispatch(&clear, &mut service).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(parsed["result"], json!({}));
    assert_eq!(service.status_at(now).unwrap()["geo_source"], "auto");
}

#[test]
fn manual_mode_fills_place_info_after_background_lookup() {
    // Regression: a manual coordinate switch must take effect immediately
    // (no network in the control path) and the country/city/timezone must
    // be filled back in once the background reverse-geocode lookup lands.
    let now = 1_000_000;
    let dir = std::env::temp_dir();
    let status_path = dir.join(format!("wloc-test-fill-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&status_path);
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    )
    .with_state_files(status_path.clone(), dir.join("wloc-test-fill-events.jsonl"));

    let params = RequestParams {
        query: None,
        latitude: Some(22.3193),
        longitude: Some(114.1694),
    };
    service.set_manual_location(&params).unwrap();
    service.status().unwrap();

    // Coordinates are effective immediately; place info is pending.
    let status = read_status_json(&status_path);
    assert_eq!(status["geo_source"], "manual");
    assert_eq!(status["geo"]["latitude"], 22.3193);
    assert!(status["geo"]["country_code"].is_null());

    // The background lookup completes successfully; the next status cycle
    // must expose country/city/timezone again.
    let generation = service.manual_geo_generation();
    service.finish_manual_geo_lookup(
        generation,
        Ok(crate_geo_result("Hong Kong", "HK", "Asia/Hong_Kong")),
    );
    service.status().unwrap();
    let status = read_status_json(&status_path);
    assert_eq!(status["geo"]["country_code"], "HK");
    assert_eq!(status["geo"]["city"], "Hong Kong");
    assert_eq!(status["geo"]["timezone"], "Asia/Hong_Kong");
    assert_eq!(status["geo"]["latitude"], 22.3193);
    let _ = std::fs::remove_file(&status_path);
}

#[test]
fn stale_lookup_result_is_discarded_after_coordinate_change() {
    // A lookup started for coordinates A must not overwrite the place info
    // of a newer manual target B when it completes late.
    let now = 1_000_000;
    let dir = std::env::temp_dir();
    let status_path = dir.join(format!("wloc-test-stale-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&status_path);
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    )
    .with_state_files(
        status_path.clone(),
        dir.join("wloc-test-stale-events.jsonl"),
    );

    let set_a = RequestParams {
        query: None,
        latitude: Some(22.3193),
        longitude: Some(114.1694),
    };
    service.set_manual_location(&set_a).unwrap();
    let generation_a = service.manual_geo_generation();

    let set_b = RequestParams {
        query: None,
        latitude: Some(51.5074),
        longitude: Some(-0.1278),
    };
    service.set_manual_location(&set_b).unwrap();
    assert_ne!(generation_a, service.manual_geo_generation());

    // The worker for A completes late; its outcome must be dropped.
    service.finish_manual_geo_lookup(
        generation_a,
        Ok(crate_geo_result("Hong Kong", "HK", "Asia/Hong_Kong")),
    );
    service.status().unwrap();
    let status = read_status_json(&status_path);
    assert_eq!(status["geo"]["latitude"], 51.5074);
    assert!(
        status["geo"]["country_code"].is_null(),
        "stale lookup must not populate the newer target"
    );
    let _ = std::fs::remove_file(&status_path);
}

#[test]
fn failed_lookup_keeps_coordinates_without_place_info() {
    // A failed or timed-out background lookup must never block the control
    // path and must leave the coordinates active with empty place info.
    let now = 1_000_000;
    let dir = std::env::temp_dir();
    let status_path = dir.join(format!("wloc-test-fail-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&status_path);
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    )
    .with_state_files(status_path.clone(), dir.join("wloc-test-fail-events.jsonl"));
    let params = RequestParams {
        query: None,
        latitude: Some(22.3193),
        longitude: Some(114.1694),
    };
    service.set_manual_location(&params).unwrap();
    let generation = service.manual_geo_generation();

    service.finish_manual_geo_lookup(generation, Err(()));
    service.status().unwrap();
    let status = read_status_json(&status_path);
    assert_eq!(status["geo_source"], "manual");
    assert_eq!(status["geo"]["latitude"], 22.3193);
    assert!(status["geo"]["country_code"].is_null());
    let _ = std::fs::remove_file(&status_path);
}

fn read_status_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn crate_geo_result(city: &str, country: &str, tz: &str) -> ReverseGeoResult {
    ReverseGeoResult {
        city: city.to_owned(),
        country_code: country.to_owned(),
        timezone: tz.to_owned(),
    }
}

#[test]
fn probe_failure_reason_is_exposed_in_the_status_file() {
    use wificalling_location_gateway::exitprobe::runtime::ProbeFailure;
    let now = 1_000_000;
    let dir = std::env::temp_dir();
    let status_path = dir.join(format!("wloc-test-probeerr-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&status_path);
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        SequenceProbe {
            results: vec![Err(ProbeFailure::DnsLookupFailed)],
            index: 0,
        },
        fresh_geo(now),
    )
    .with_state_files(
        status_path.clone(),
        dir.join("wloc-test-probeerr-events.jsonl"),
    );

    service.status().unwrap();
    let status: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&status_path).unwrap()).unwrap();
    assert_eq!(status["exit"]["state"], "unavailable");
    assert_eq!(status["exit"]["ip"], serde_json::Value::Null);
    assert_eq!(status["exit"]["last_error"], "node DNS resolution failed");
    let _ = std::fs::remove_file(&status_path);
}

#[test]
fn deleted_followed_node_is_exposed_and_clears_stale_location() {
    let now = 1_000_000;
    let dir = std::env::temp_dir();
    let status_path = dir.join(format!(
        "wloc-test-missing-node-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&status_path);
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        SequenceProbe {
            results: vec![Err(ProbeFailure::BoundNodeMissing)],
            index: 0,
        },
        fresh_geo(now),
    )
    .with_state_files(
        status_path.clone(),
        dir.join("wloc-test-missing-node-events.jsonl"),
    );

    service.status().unwrap();
    let status = read_status_json(&status_path);
    assert_eq!(status["exit"]["state"], "unavailable");
    assert_eq!(status["exit"]["ip"], serde_json::Value::Null);
    assert_eq!(status["geo"]["state"], "unavailable");
    assert_eq!(status["geo"]["latitude"], serde_json::Value::Null);
    assert_eq!(
        status["exit"]["last_error"],
        "followed device node is missing; select and apply a WCG node"
    );
    let _ = std::fs::remove_file(&status_path);
}

fn limits() -> ProbeLimits {
    ProbeLimits {
        max_observation_age: Duration::from_secs(60),
    }
}

fn record(now_unix: u64, latitude: f64, longitude: f64) -> GeoRecord {
    GeoRecord {
        country_code: "US".to_owned(),
        city: "Ashburn".to_owned(),
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
            assigned_device: Some("192.168.31.176".to_owned()),
            // Unit tests drive the lookup via finish_manual_geo_lookup; no
            // real network I/O.
            reverse_geo_lookup: None,
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
            assigned_device: Some("192.168.31.176".to_owned()),
            // Unit tests drive the lookup via finish_manual_geo_lookup; no
            // real network I/O.
            reverse_geo_lookup: None,
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
fn control_refresh_forces_an_immediate_reprobe_of_fresh_evidence() {
    // Regression: switching the followed device's node in the Gateway
    // settings must be reflected in the exit IP without waiting for the
    // periodic housekeeping tick. The manual refresh command discards
    // cached evidence and probes again even though the first observation
    // is still fresh; the status file the monitor reads shows the new IP.
    use serde_json::json;
    use wificalling_location_gateway::service::api::{decode_request, SERVICE_API_ID};
    use wificalling_location_gateway::service::dispatch::dispatch;

    let now = 1_000_000;
    let dir = std::env::temp_dir();
    let status_path = dir.join(format!("wloc-test-refresh-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&status_path);
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
                Ok(Some((EXIT_B, record(now, 22.32, 114.17)))),
            ],
            index: 0,
        },
    )
    .with_state_files(
        status_path.clone(),
        dir.join("wloc-test-refresh-events.jsonl"),
    );

    // First probe observes EXIT_A.
    service.status_at(now).unwrap();
    let first = read_status_json(&status_path);
    assert_eq!(first["exit"]["ip"], json!(EXIT_A.to_string()));

    // The monitor's refresh command re-probes immediately.
    let refresh = decode_request(
        &serde_json::to_vec(&json!({
            "api_version": SERVICE_API_ID,
            "request_id": "req-refresh",
            "method": "control.refresh",
            "params": {}
        }))
        .unwrap(),
    )
    .unwrap();
    let response = dispatch(&refresh, &mut service).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(parsed["result"], json!({}));

    // The status file now carries the new exit IP. (The forced probe runs
    // against the real clock while the test records use a fake clock, so
    // the geo record expires and geo is unavailable - the exit IP is the
    // contract under test.)
    let refreshed = read_status_json(&status_path);
    assert_eq!(refreshed["exit"]["ip"], json!(EXIT_B.to_string()));

    // The refreshed evidence is fresh: a follow-up status call does not
    // probe a third time (the sequence probe would run out of results).
    let again = service.status_at(now).unwrap();
    assert_eq!(again["exit"]["state"], "verified");
    let _ = std::fs::remove_file(&status_path);
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

#[test]
fn manual_location_overrides_auto_patch_target() {
    let now = 1_000_000;
    let sink = Arc::new(Mutex::new(None));
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    )
    .with_patch_sink(Arc::clone(&sink));

    // Auto mode publishes the Geo record for the stub exit.
    service.status_at(now).unwrap();
    assert!(
        sink.lock().unwrap().is_some(),
        "auto mode must publish a target"
    );
    assert_eq!(service.status_at(now).unwrap()["geo_source"], "auto");

    // Set a manual preset by explicit coordinates; the sink now publishes it.
    let params = RequestParams {
        query: None,
        latitude: Some(51.5074),
        longitude: Some(-0.1278),
    };
    service.set_manual_location(&params).unwrap();
    let published = *sink.lock().unwrap();
    let (lat, lon) = published.map(|t| (t.latitude, t.longitude)).unwrap();
    assert_eq!((lat, lon), (51.5074, -0.1278));
    assert_eq!(service.status_at(now).unwrap()["geo_source"], "manual");

    // Clear returns to automatic node-following.
    service.clear_manual_location().unwrap();
    assert_eq!(service.status_at(now).unwrap()["geo_source"], "auto");
}

#[test]
fn invalid_manual_location_is_rejected() {
    let now = 1_000_000;
    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    );
    // Latitude out of range must be rejected.
    let bad = RequestParams {
        query: None,
        latitude: Some(95.0),
        longitude: Some(0.0),
    };
    assert_eq!(
        service.set_manual_location(&bad),
        Err(DispatchError::InvalidLocation)
    );
    // Neither a query nor coordinates is invalid.
    let empty = RequestParams {
        query: None,
        latitude: None,
        longitude: None,
    };
    assert_eq!(
        service.set_manual_location(&empty),
        Err(DispatchError::InvalidLocation)
    );
}

#[test]
fn status_file_and_target_events_are_written() {
    let now = 1_000_000;
    let dir = std::env::temp_dir();
    let status_path = dir.join("wloc-test-status.json");
    let events_path = dir.join("wloc-test-events.jsonl");
    let _ = std::fs::remove_file(&status_path);
    let _ = std::fs::remove_file(&events_path);

    let mut service = build(
        OkRuntime {
            healthy: true,
            install_fails: false,
        },
        fresh_probe(),
        fresh_geo(now),
    )
    .with_state_files(status_path.clone(), events_path.clone());

    service.status_at(now).unwrap();
    // Manual location publishes a target_updated event.
    let params = RequestParams {
        query: None,
        latitude: Some(51.5074),
        longitude: Some(-0.1278),
    };
    service.set_manual_location(&params).unwrap();

    let status_text = std::fs::read_to_string(&status_path).unwrap();
    let status: serde_json::Value = serde_json::from_str(&status_text).unwrap();
    assert_eq!(status["geo_source"], "manual");
    assert!(
        status["geo"]["latitude"].is_number(),
        "status file must carry GPS"
    );
    // A manual preset is the effective target. The control operation is
    // deliberately local-only, so optional place metadata remains null.
    assert_eq!(status["geo"]["latitude"], 51.5074);
    assert_eq!(status["geo"]["longitude"], -0.1278);
    assert!(status["geo"]["country_code"].is_null());
    assert!(status["geo"]["city"].is_null());
    assert!(status["geo"]["timezone"].is_null());

    let events_text = std::fs::read_to_string(&events_path).unwrap();
    assert!(
        events_text.contains("target_updated"),
        "events must record target updates"
    );

    let _ = std::fs::remove_file(&status_path);
    let _ = std::fs::remove_file(&events_path);
}
