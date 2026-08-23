//! Exit-probe runtime adapter contract.
//!
//! The pure [`validate_observation`] logic is driven by a runtime adapter that
//! probes the bound node's public exit IP and reports the router's verified
//! WAN addresses. Any probe failure is fail-closed: no observation is ever
//! produced without a successful probe.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use wificalling_location_gateway::exitprobe::runtime::{
    observe_exit, ExitProbeRuntime, ProbeFailure,
};
use wificalling_location_gateway::exitprobe::{ExitProbeError, NodeRef, ProbeLimits};

struct StubProbe {
    probe_result: Result<IpAddr, ProbeFailure>,
    wan_result: Result<Vec<IpAddr>, ProbeFailure>,
}

impl ExitProbeRuntime for StubProbe {
    fn probe_exit_ip(&mut self) -> Result<IpAddr, ProbeFailure> {
        self.probe_result
    }
    fn router_wan_ips(&mut self) -> Result<Vec<IpAddr>, ProbeFailure> {
        self.wan_result.clone()
    }
}

fn limits() -> ProbeLimits {
    ProbeLimits {
        max_observation_age: Duration::from_secs(120),
    }
}

const PUBLIC_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
const OTHER_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
const PRIVATE_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
const PUBLIC_V6: IpAddr = IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111));

#[test]
fn successful_probe_produces_a_validated_observation() {
    let mut probe = StubProbe {
        probe_result: Ok(PUBLIC_V4),
        wan_result: Ok(vec![OTHER_V4]),
    };
    let node = NodeRef::new("node-1").unwrap();
    let observation = observe_exit(&mut probe, node.clone(), 1_000, limits()).unwrap();

    assert_eq!(observation.node_ref(), &node);
    assert_eq!(observation.exit_ip(), PUBLIC_V4);
    assert_eq!(observation.checked_at_unix(), 1_000);
}

#[test]
fn probe_failure_is_fail_closed() {
    for failure in [
        ProbeFailure::BoundNodeMissing,
        ProbeFailure::Unreachable,
        ProbeFailure::Timeout,
    ] {
        let mut probe = StubProbe {
            probe_result: Err(failure),
            wan_result: Ok(vec![OTHER_V4]),
        };
        let node = NodeRef::new("node-1").unwrap();
        assert_eq!(
            observe_exit(&mut probe, node, 1_000, limits()),
            Err(ExitProbeError::Probe(failure))
        );
    }
}

#[test]
fn wan_failure_is_fail_closed() {
    let mut probe = StubProbe {
        probe_result: Ok(PUBLIC_V4),
        wan_result: Err(ProbeFailure::Unreachable),
    };
    let node = NodeRef::new("node-1").unwrap();
    assert_eq!(
        observe_exit(&mut probe, node, 1_000, limits()),
        Err(ExitProbeError::RuntimeFailure)
    );
}

#[test]
fn direct_wan_address_is_never_accepted_as_a_proxy_exit() {
    let mut probe = StubProbe {
        probe_result: Ok(PUBLIC_V4),
        wan_result: Ok(vec![PUBLIC_V4]),
    };
    let node = NodeRef::new("node-1").unwrap();
    assert_eq!(
        observe_exit(&mut probe, node, 1_000, limits()),
        Err(ExitProbeError::RouterWanAddress)
    );
}

#[test]
fn empty_wan_set_is_fail_closed() {
    let mut probe = StubProbe {
        probe_result: Ok(PUBLIC_V4),
        wan_result: Ok(vec![]),
    };
    let node = NodeRef::new("node-1").unwrap();
    assert_eq!(
        observe_exit(&mut probe, node, 1_000, limits()),
        Err(ExitProbeError::RouterWanUnknown)
    );
}

#[test]
fn non_public_probe_address_is_rejected() {
    let mut probe = StubProbe {
        probe_result: Ok(PRIVATE_V4),
        wan_result: Ok(vec![OTHER_V4]),
    };
    let node = NodeRef::new("node-1").unwrap();
    assert_eq!(
        observe_exit(&mut probe, node, 1_000, limits()),
        Err(ExitProbeError::NonPublicAddress)
    );
}

#[test]
fn opposite_family_wan_cannot_validate_the_exit() {
    // A single IPv4 WAN address must not validate an IPv6 exit observation.
    let mut probe = StubProbe {
        probe_result: Ok(PUBLIC_V6),
        wan_result: Ok(vec![OTHER_V4]),
    };
    let node = NodeRef::new("node-1").unwrap();
    assert_eq!(
        observe_exit(&mut probe, node, 1_000, limits()),
        Err(ExitProbeError::RouterWanUnknown)
    );
}

#[test]
fn boxed_probe_forwards_to_the_inner_implementation() {
    let mut probe: Box<dyn ExitProbeRuntime> = Box::new(StubProbe {
        probe_result: Ok(PUBLIC_V4),
        wan_result: Ok(vec![OTHER_V4]),
    });
    assert_eq!(probe.probe_exit_ip(), Ok(PUBLIC_V4));
    assert_eq!(probe.router_wan_ips(), Ok(vec![OTHER_V4]));
}
