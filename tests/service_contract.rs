use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use wificalling_location_gateway::service::{
    current_response_mode, decide_ingress, GeoRecord, IngressDisposition, ResponseMode,
    RuntimeHealth, ServiceConfig, ServiceConfigError, TrafficMeta, Transport, SERVICE_API_VERSION,
};
use wificalling_location_gateway::APPROVED_WLOC_HOSTS;

#[test]
fn approved_wloc_hosts_are_exactly_the_two_apple_names() {
    assert_eq!(
        APPROVED_WLOC_HOSTS,
        ["gs-loc.apple.com", "gs-loc-cn.apple.com"]
    );
}

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
        city: "London".to_owned(),
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
    assert_eq!(
        config.validate(),
        Err(ServiceConfigError::InvalidMaxConnections)
    );

    let mut config = valid_config();
    config.failure_grace = Duration::from_secs(31);
    assert_eq!(
        config.validate(),
        Err(ServiceConfigError::InvalidFailureGrace)
    );
}

#[test]
fn only_the_assigned_device_exact_hosts_and_tcp_443_route_to_mitm() {
    let config = valid_config();

    assert_eq!(
        decide_ingress(&config, &approved_traffic(), RuntimeHealth::Healthy),
        IngressDisposition::RouteToMitm
    );

    let mut wrong_device = approved_traffic();
    wrong_device.source_ip = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 11));
    assert_eq!(
        decide_ingress(&config, &wrong_device, RuntimeHealth::Healthy),
        IngressDisposition::BypassMitm
    );

    for hostname in ["GS-LOC.APPLE.COM", "gs-loc.apple.com.invalid", "apple.com"] {
        let mut traffic = approved_traffic();
        traffic.hostname = hostname.to_owned();
        assert_eq!(
            decide_ingress(&config, &traffic, RuntimeHealth::Healthy),
            IngressDisposition::BypassMitm
        );
    }

    let mut udp = approved_traffic();
    udp.transport = Transport::Udp;
    udp.destination_port = 4500;
    assert_eq!(
        decide_ingress(&config, &udp, RuntimeHealth::Healthy),
        IngressDisposition::BypassMitm
    );
}

#[test]
fn geo_validation_is_separate_from_ingress_and_patch_stays_disabled() {
    let mut stale = fresh_geo();
    stale.expires_at_unix = 999;
    assert!(stale.validate_at(1_000).is_err());

    for invalid in [
        GeoRecord {
            latitude: 91.0,
            ..fresh_geo()
        },
        GeoRecord {
            longitude: 181.0,
            ..fresh_geo()
        },
        GeoRecord {
            country_code: "GBR".to_owned(),
            ..fresh_geo()
        },
        GeoRecord {
            timezone: "".to_owned(),
            ..fresh_geo()
        },
    ] {
        assert!(invalid.validate_at(1_000).is_err());
    }

    assert!(fresh_geo().validate_at(1_000).is_ok());
    assert_eq!(current_response_mode(), ResponseMode::ForwardOriginal);
}

#[test]
fn disabled_service_bypasses_and_unhealthy_engine_withdraws_redirect() {
    let mut disabled = valid_config();
    disabled.enabled = false;
    assert_eq!(
        decide_ingress(&disabled, &approved_traffic(), RuntimeHealth::Healthy),
        IngressDisposition::BypassMitm
    );

    assert_eq!(
        decide_ingress(
            &valid_config(),
            &approved_traffic(),
            RuntimeHealth::Unhealthy,
        ),
        IngressDisposition::WithdrawRedirect
    );
}
