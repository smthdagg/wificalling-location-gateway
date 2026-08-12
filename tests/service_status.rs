use wificalling_location_gateway::service::state::ServiceState;
use wificalling_location_gateway::service::status::{
    encode_status, DesiredState, EngineHealth, ExitState, GeoSourceState, GeoState, StatusInputs,
};

fn disabled_inputs() -> StatusInputs {
    StatusInputs {
        generation: 7,
        observed_at_unix: 123,
        desired_state: DesiredState::Disabled,
        service_state: ServiceState::disabled(),
        engine_health: EngineHealth::Stopped,
        engine_uptime_seconds: 0,
        assigned_device_configured: false,
        exit_state: ExitState::Unknown,
        exit_checked_at: None,
        geo_state: GeoState::Unavailable,
        geo_expires_at: None,
        geo_source: GeoSourceState::Auto,
    }
}

#[test]
fn disabled_status_matches_the_frozen_coordinate_free_contract() {
    let encoded = encode_status(&disabled_inputs()).expect("bounded status must encode");

    assert_eq!(
        String::from_utf8(encoded).expect("status is UTF-8 JSON"),
        r#"{"api_version":"wloc.service/v1","generation":7,"observed_at":123,"desired_state":"disabled","service_phase":"disabled","safety":{"redirect_present":false,"watchdog_armed":false,"scope_valid":false,"ipv6_ready":false,"response_mode":"forward_original"},"engine":{"health":"stopped","uptime_seconds":0},"exit":{"state":"unknown","checked_at":null},"geo":{"state":"unavailable","expires_at":null},"geo_source":"auto","assigned_device_configured":false,"last_error":null}"#
    );
}

#[test]
fn default_status_cannot_expose_device_or_location_material() {
    let encoded = encode_status(&StatusInputs {
        generation: u64::MAX,
        observed_at_unix: u64::MAX,
        desired_state: DesiredState::Enabled,
        service_state: ServiceState::disabled(),
        engine_health: EngineHealth::Unhealthy,
        engine_uptime_seconds: u64::MAX,
        assigned_device_configured: true,
        exit_state: ExitState::Verified,
        exit_checked_at: Some(u64::MAX),
        geo_state: GeoState::Fresh,
        geo_expires_at: Some(u64::MAX),
        geo_source: GeoSourceState::Manual,
    })
    .expect("bounded status must encode");
    let status = String::from_utf8(encoded).expect("status is UTF-8 JSON");

    for forbidden in [
        "latitude",
        "longitude",
        "device_ip",
        "device_mac",
        "credential",
        "private_key",
        "provider_payload",
    ] {
        assert!(!status.contains(forbidden), "leaked field: {forbidden}");
    }
    assert!(status.contains(r#""assigned_device_configured":true"#));
    assert!(status.contains(r#""response_mode":"forward_original""#));
}

#[test]
fn verified_exit_and_fresh_geo_are_reported_without_coordinates() {
    let encoded = encode_status(&StatusInputs {
        exit_state: ExitState::Verified,
        exit_checked_at: Some(4_000_000),
        geo_state: GeoState::Fresh,
        geo_expires_at: Some(4_003_600),
        ..disabled_inputs()
    })
    .expect("bounded status must encode");
    let status = String::from_utf8(encoded).expect("status is UTF-8 JSON");

    assert!(status.contains(r#""exit":{"state":"verified","checked_at":4000000}"#));
    assert!(status.contains(r#""geo":{"state":"fresh","expires_at":4003600}"#));
    assert!(
        !status.contains("latitude") && !status.contains("longitude"),
        "fresh geo evidence must never carry coordinates"
    );
}

#[test]
fn stale_exit_and_uncertain_geo_are_reported_with_their_states() {
    let encoded = encode_status(&StatusInputs {
        exit_state: ExitState::Stale,
        exit_checked_at: Some(1),
        geo_state: GeoState::Uncertain,
        geo_expires_at: None,
        ..disabled_inputs()
    })
    .expect("bounded status must encode");
    let status = String::from_utf8(encoded).expect("status is UTF-8 JSON");

    assert!(status.contains(r#""exit":{"state":"stale","checked_at":1}"#));
    assert!(status.contains(r#""geo":{"state":"uncertain","expires_at":null}"#));
}

#[test]
fn unavailable_exit_state_is_the_fail_closed_wire_value() {
    let encoded = encode_status(&StatusInputs {
        exit_state: ExitState::Unavailable,
        exit_checked_at: None,
        ..disabled_inputs()
    })
    .expect("bounded status must encode");
    let status = String::from_utf8(encoded).expect("status is UTF-8 JSON");
    assert!(status.contains(r#""exit":{"state":"unavailable","checked_at":null}"#));
}
