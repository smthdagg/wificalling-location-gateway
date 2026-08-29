//! Validation boundary for exit observations.
//!
//! Network execution uses the Gateway's existing sing-box probe listeners. This
//! module accepts only a bounded, non-secret node reference and a fresh public
//! address that differs from the router WAN address.

pub mod runtime;
pub mod singbox;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

const MAX_NODE_REF_BYTES: usize = 64;
/// Hard ceiling for an observation's accepted age. UCI `probe_interval` must
/// be clamped to this value: a larger configured interval would make every
/// observation fail `InvalidLimits` and permanently block auto-mode enable.
pub const MAX_OBSERVATION_AGE: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeRef(String);

impl NodeRef {
    pub fn new(value: &str) -> Result<Self, ExitProbeError> {
        if value.is_empty()
            || value.len() > MAX_NODE_REF_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(ExitProbeError::InvalidNodeRef);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProbeLimits {
    pub max_observation_age: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExitObservation {
    node_ref: NodeRef,
    exit_ip: IpAddr,
    checked_at_unix: u64,
}

impl ExitObservation {
    pub fn node_ref(&self) -> &NodeRef {
        &self.node_ref
    }

    pub const fn exit_ip(&self) -> IpAddr {
        self.exit_ip
    }

    pub const fn checked_at_unix(&self) -> u64 {
        self.checked_at_unix
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitProbeError {
    InvalidNodeRef,
    InvalidLimits,
    NonPublicAddress,
    RouterWanUnknown,
    RouterWanAddress,
    ObservationFromFuture,
    StaleObservation,
    RuntimeFailure,
    /// The exit probe itself failed; carries the classified reason.
    Probe(crate::exitprobe::runtime::ProbeFailure),
}

impl std::fmt::Display for ExitProbeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExitProbeError::InvalidNodeRef => formatter.write_str("invalid node reference"),
            ExitProbeError::InvalidLimits => formatter.write_str("invalid probe limits"),
            ExitProbeError::NonPublicAddress => formatter.write_str("exit IP is not public"),
            ExitProbeError::RouterWanUnknown => formatter.write_str("router WAN address unknown"),
            ExitProbeError::RouterWanAddress => {
                formatter.write_str("exit IP equals the router WAN")
            }
            ExitProbeError::ObservationFromFuture => {
                formatter.write_str("observation from the future")
            }
            ExitProbeError::StaleObservation => formatter.write_str("stale observation"),
            ExitProbeError::RuntimeFailure => formatter.write_str("probe runtime failure"),
            ExitProbeError::Probe(failure) => formatter.write_str(failure.message()),
        }
    }
}

pub fn validate_observation(
    node_ref: NodeRef,
    observed_ip: IpAddr,
    router_wan_ips: &[IpAddr],
    checked_at_unix: u64,
    now_unix: u64,
    limits: ProbeLimits,
) -> Result<ExitObservation, ExitProbeError> {
    if limits.max_observation_age.is_zero() || limits.max_observation_age > MAX_OBSERVATION_AGE {
        return Err(ExitProbeError::InvalidLimits);
    }
    if !is_public_address(observed_ip) {
        return Err(ExitProbeError::NonPublicAddress);
    }
    if !router_wan_ips
        .iter()
        .any(|known| same_address_family(*known, observed_ip))
    {
        return Err(ExitProbeError::RouterWanUnknown);
    }
    if router_wan_ips.contains(&observed_ip) {
        return Err(ExitProbeError::RouterWanAddress);
    }
    if checked_at_unix > now_unix {
        return Err(ExitProbeError::ObservationFromFuture);
    }
    if now_unix - checked_at_unix > limits.max_observation_age.as_secs() {
        return Err(ExitProbeError::StaleObservation);
    }

    Ok(ExitObservation {
        node_ref,
        exit_ip: observed_ip,
        checked_at_unix,
    })
}

fn is_public_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && (c == 0 || c == 2))
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    let global_unicast = (segments[0] & 0xe000) == 0x2000;
    let special_2001_low = segments[0] == 0x2001 && segments[1] <= 0x01ff;
    let documentation = (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || (segments[0] == 0x3fff && segments[1] <= 0x0fff);
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || !global_unicast
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || special_2001_low
        || documentation
        || segments[0] == 0x2002
        || address.to_ipv4_mapped().is_some())
}

const fn same_address_family(left: IpAddr, right: IpAddr) -> bool {
    matches!(
        (left, right),
        (IpAddr::V4(_), IpAddr::V4(_)) | (IpAddr::V6(_), IpAddr::V6(_))
    )
}
