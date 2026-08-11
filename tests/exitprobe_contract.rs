use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use wificalling_location_gateway::exitprobe::{
    validate_observation, ExitProbeError, NodeRef, ProbeLimits,
};

fn limits() -> ProbeLimits {
    ProbeLimits {
        max_observation_age: Duration::from_secs(30),
    }
}

#[test]
fn verified_observation_records_only_a_non_secret_node_reference_and_public_exit() {
    let node = NodeRef::new("uk_anytls-1").expect("safe node reference");
    let exit = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
    let observation = validate_observation(
        node,
        exit,
        &[IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10))],
        1_990,
        2_000,
        limits(),
    )
    .expect("fresh public exit must validate");

    assert_eq!(observation.node_ref().as_str(), "uk_anytls-1");
    assert_eq!(observation.exit_ip(), exit);
    assert_eq!(observation.checked_at_unix(), 1_990);
}

#[test]
fn unsafe_node_references_and_probe_limits_are_rejected() {
    for value in ["", "node secret", "ss://credential", "user@example.com"] {
        assert_eq!(
            NodeRef::new(value).unwrap_err(),
            ExitProbeError::InvalidNodeRef
        );
    }

    let node = NodeRef::new("node-1").expect("safe node reference");
    assert_eq!(
        validate_observation(
            node,
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            &[IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10))],
            1_000,
            1_000,
            ProbeLimits {
                max_observation_age: Duration::ZERO,
            },
        )
        .unwrap_err(),
        ExitProbeError::InvalidLimits
    );
}

#[test]
fn private_reserved_and_router_wan_addresses_are_not_accepted_as_proxy_exits() {
    let rejected = [
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(169, 254, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        "fc00::1".parse().expect("valid IPv6"),
        "fe80::1".parse().expect("valid IPv6"),
        "2001:db8::1".parse().expect("valid IPv6"),
    ];

    for exit in rejected {
        assert_eq!(
            validate_observation(
                NodeRef::new("node-1").expect("safe node reference"),
                exit,
                &[IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10))],
                1_000,
                1_000,
                limits(),
            )
            .unwrap_err(),
            ExitProbeError::NonPublicAddress
        );
    }

    let wan = IpAddr::V6("2606:4700:4700::1111".parse().expect("valid IPv6"));
    assert_eq!(
        validate_observation(
            NodeRef::new("node-1").expect("safe node reference"),
            wan,
            &[wan],
            1_000,
            1_000,
            limits(),
        )
        .unwrap_err(),
        ExitProbeError::RouterWanAddress
    );
}

#[test]
fn stale_or_future_observations_fail_closed() {
    let public = IpAddr::V4(Ipv4Addr::new(8, 8, 4, 4));
    for (checked_at, now, expected) in [
        (969, 1_000, ExitProbeError::StaleObservation),
        (1_001, 1_000, ExitProbeError::ObservationFromFuture),
    ] {
        assert_eq!(
            validate_observation(
                NodeRef::new("node-1").expect("safe node reference"),
                public,
                &[IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10))],
                checked_at,
                now,
                limits(),
            )
            .unwrap_err(),
            expected
        );
    }
}

#[test]
fn router_wan_must_be_known_for_the_observed_address_family() {
    for (exit, known_wan) in [
        (IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), Vec::new()),
        (
            IpAddr::V6("2606:4700:4700::1111".parse().expect("valid IPv6")),
            vec![IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10))],
        ),
    ] {
        assert_eq!(
            validate_observation(
                NodeRef::new("node-1").expect("safe node reference"),
                exit,
                &known_wan,
                1_000,
                1_000,
                limits(),
            )
            .unwrap_err(),
            ExitProbeError::RouterWanUnknown
        );
    }
}

#[test]
fn special_purpose_ipv6_ranges_are_not_proxy_exits() {
    for value in [
        "::8.8.8.8",
        "100::1",
        "2001:2::1",
        "2002::1",
        "3fff::1",
    ] {
        assert_eq!(
            validate_observation(
                NodeRef::new("node-1").expect("safe node reference"),
                IpAddr::V6(value.parse().expect("valid IPv6")),
                &[IpAddr::V6(
                    "2606:4700:4700::1001".parse().expect("valid IPv6"),
                )],
                1_000,
                1_000,
                limits(),
            )
            .unwrap_err(),
            ExitProbeError::NonPublicAddress,
            "unexpectedly accepted {value}"
        );
    }
}
