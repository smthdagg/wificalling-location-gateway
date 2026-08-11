use wificalling_location_gateway::service::state::ServiceState;
use wificalling_location_gateway::service::status::{
    encode_status, DesiredState, EngineHealth, StatusInputs,
};

#[test]
fn disabled_status_matches_the_frozen_coordinate_free_contract() {
    let encoded = encode_status(&StatusInputs {
        generation: 7,
        observed_at_unix: 123,
        desired_state: DesiredState::Disabled,
        service_state: ServiceState::disabled(),
        engine_health: EngineHealth::Stopped,
        engine_uptime_seconds: 0,
        assigned_device_configured: false,
    })
    .expect("bounded status must encode");

    assert_eq!(
        String::from_utf8(encoded).expect("status is UTF-8 JSON"),
        r#"{"api_version":"wloc.service/v1","generation":7,"observed_at":123,"desired_state":"disabled","service_phase":"disabled","safety":{"redirect_present":false,"watchdog_armed":false,"scope_valid":false,"ipv6_ready":false,"response_mode":"forward_original"},"engine":{"health":"stopped","uptime_seconds":0},"exit":{"state":"unknown","checked_at":null},"geo":{"state":"unavailable","expires_at":null},"assigned_device_configured":false,"last_error":null}"#
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
