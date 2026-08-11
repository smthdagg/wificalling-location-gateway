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
    /// The probe target or exit path is unreachable.
    Unreachable,
    /// The probe exceeded its bounded deadline.
    Timeout,
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
    let observed_ip = runtime
        .probe_exit_ip()
        .map_err(|_| ExitProbeError::RuntimeFailure)?;
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
