use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use wificalling_location_gateway::service::{
    decide_routing, GeoRecord, RoutingAction, RuntimeHealth, ServiceConfig, ServiceConfigError,
    TrafficMeta, Transport, SERVICE_API_VERSION,
};

fn assigned_device() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10))
}

fn valid_config() -> ServiceConfig {
    ServiceConfig {
        enabled: true,
        assigned_device: assigned_device(),
        max_connections: 4,
        failure_grace: Duration::from_secs(5),
    }
}

fn approved_traffic() -> TrafficMeta {
    TrafficMeta {
        source_ip: assigned_device(),
        hostname: "gs-loc.apple.com".to_owned(),
        transport: Transport::Tcp,
        destination_port: 443,
    }
}

fn fresh_geo() -> GeoRecord {
    GeoRecord {
        country_code: "GB".to_owned(),
        latitude: 51.5074,
        longitude: -0.1278,
        timezone: "Europe/London".to_owned(),
        expires_at_unix: 2_000,
    }
}

#[test]
fn service_contract_has_a_stable_version_for_future_ui_clients() {
    assert_eq!(SERVICE_API_VERSION, 1);
}

#[test]
fn invalid_resource_configuration_is_rejected() {
    let mut config = valid_config();
    config.max_connections = 0;
    assert_eq!(config.validate(), Err(ServiceConfigError::InvalidMaxConnections));

    let mut config = valid_config();
    config.failure_grace = Duration::from_secs(31);
    assert_eq!(config.validate(), Err(ServiceConfigError::InvalidFailureGrace));
}

#[test]
fn only_the_assigned_device_exact_hosts_and_tcp_443_can_be_intercepted() {
    let config = valid_config();
    let geo = fresh_geo();

    assert_eq!(
        decide_routing(&config, &approved_traffic(), Some(&geo), RuntimeHealth::Healthy, 1_000),
        RoutingAction::Intercept
    );

    let mut wrong_device = approved_traffic();
    wrong_device.source_ip = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 11));
    assert_eq!(
        decide_routing(&config, &wrong_device, Some(&geo), RuntimeHealth::Healthy, 1_000),
        RoutingAction::PassThrough
    );

    for hostname in ["GS-LOC.APPLE.COM", "gs-loc.apple.com.invalid", "apple.com"] {
        let mut traffic = approved_traffic();
        traffic.hostname = hostname.to_owned();
        assert_eq!(
            decide_routing(&config, &traffic, Some(&geo), RuntimeHealth::Healthy, 1_000),
            RoutingAction::PassThrough
        );
    }

    let mut udp = approved_traffic();
    udp.transport = Transport::Udp;
    udp.destination_port = 4500;
    assert_eq!(
        decide_routing(&config, &udp, Some(&geo), RuntimeHealth::Healthy, 1_000),
        RoutingAction::PassThrough
    );
}

#[test]
fn stale_or_invalid_geo_never_produces_an_intercept_decision() {
    let config = valid_config();
    let traffic = approved_traffic();

    let mut stale = fresh_geo();
    stale.expires_at_unix = 999;
    assert_eq!(
        decide_routing(&config, &traffic, Some(&stale), RuntimeHealth::Healthy, 1_000),
        RoutingAction::PassThrough
    );

    for invalid in [
        GeoRecord { latitude: 91.0, ..fresh_geo() },
        GeoRecord { longitude: 181.0, ..fresh_geo() },
        GeoRecord { country_code: "GBR".to_owned(), ..fresh_geo() },
        GeoRecord { timezone: "".to_owned(), ..fresh_geo() },
    ] {
        assert_eq!(
            decide_routing(&config, &traffic, Some(&invalid), RuntimeHealth::Healthy, 1_000),
            RoutingAction::PassThrough
        );
    }
}

#[test]
fn disabled_service_passes_through_and_unhealthy_engine_removes_redirect() {
    let mut disabled = valid_config();
    disabled.enabled = false;
    assert_eq!(
        decide_routing(
            &disabled,
            &approved_traffic(),
            Some(&fresh_geo()),
            RuntimeHealth::Healthy,
            1_000,
        ),
        RoutingAction::PassThrough
    );

    assert_eq!(
        decide_routing(
            &valid_config(),
            &approved_traffic(),
            Some(&fresh_geo()),
            RuntimeHealth::Unhealthy,
            1_000,
        ),
        RoutingAction::RemoveRedirect
    );
}
