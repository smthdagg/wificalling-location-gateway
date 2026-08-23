//! Runtime boundary behind exit probing.
//!
//! The pure validation logic in the parent module is driven by an adapter that
//! performs the actual network work. A probe or WAN failure is fail-closed:
//! [`observe_exit`] never produces an observation unless both the probe and
//! the verified WAN set are available, so a direct-connect address can never
//! masquerade as a verified proxy exit.

use std::net::IpAddr;

use super::{validate_observation, ExitObservation, ExitProbeError, NodeRef, ProbeLimits};

/// Failures surfaced by the underlying probe mechanism. The adapter may use
/// these for its own diagnostics; the orchestrator maps them all to
/// [`ExitProbeError::RuntimeFailure`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeFailure {
    /// The followed device or its bound node is absent from the current
    /// Gateway configuration. Selecting another node would publish a false
    /// location, so this condition is always fail-closed.
    BoundNodeMissing,
    /// The probe target or exit path is unreachable.
    Unreachable,
    /// The probe exceeded its bounded deadline.
    Timeout,
    /// The probe returned unusable or malformed data.
    InvalidData,
    /// The probe could not resolve the node's server name (DNS failure).
    DnsLookupFailed,
}

impl ProbeFailure {
    /// User-facing reason, shown in the monitor when the exit IP is unknown.
    pub fn message(self) -> &'static str {
        match self {
            ProbeFailure::BoundNodeMissing => {
                "followed device node is missing; select and apply a WCG node"
            }
            ProbeFailure::Unreachable => "node unreachable",
            ProbeFailure::Timeout => "node connection timed out",
            ProbeFailure::InvalidData => "invalid probe response",
            ProbeFailure::DnsLookupFailed => "node DNS resolution failed",
        }
    }
}

/// Network execution boundary for exit probing.
///
/// Implementations talk to sing-box or an equivalent probe mechanism on the
/// router. They must not retain credentials, raw captures, or device
/// identifiers.
pub trait ExitProbeRuntime {
    /// Probe the current public exit IP of the bound node.
    fn probe_exit_ip(&mut self) -> Result<IpAddr, ProbeFailure>;
    /// Report the router's verified WAN addresses (all known families).
    fn router_wan_ips(&mut self) -> Result<Vec<IpAddr>, ProbeFailure>;
    /// Fingerprint of the probe's backing configuration (the Gateway
    /// sing-box.json). A change means the device's node was switched, so
    /// even fresh evidence must be re-probed. `None` disables the check.
    fn config_fingerprint(&mut self) -> Option<u64> {
        None
    }
}

impl ExitProbeRuntime for Box<dyn ExitProbeRuntime> {
    fn probe_exit_ip(&mut self) -> Result<IpAddr, ProbeFailure> {
        (**self).probe_exit_ip()
    }
    fn router_wan_ips(&mut self) -> Result<Vec<IpAddr>, ProbeFailure> {
        (**self).router_wan_ips()
    }
    fn config_fingerprint(&mut self) -> Option<u64> {
        (**self).config_fingerprint()
    }
}

/// Probe the node exit and validate it against the router WAN set.
///
/// The observation timestamp is the caller's `now_unix`; a probe or WAN
/// failure never produces an observation.
pub fn observe_exit(
    runtime: &mut impl ExitProbeRuntime,
    node_ref: NodeRef,
    now_unix: u64,
    limits: ProbeLimits,
) -> Result<ExitObservation, ExitProbeError> {
    let observed_ip = runtime.probe_exit_ip().map_err(ExitProbeError::Probe)?;
    let router_wan_ips = runtime
        .router_wan_ips()
        .map_err(|_| ExitProbeError::RuntimeFailure)?;
    validate_observation(
        node_ref,
        observed_ip,
        &router_wan_ips,
        now_unix,
        now_unix,
        limits,
    )
}
