//! Runtime boundary behind Geo resolution.
//!
//! The pure candidate-resolution logic in the parent module is driven by a
//! provider adapter that performs the real network lookups. A provider that
//! fails, returns no data, returns data for the wrong exit, or returns a
//! record that fails validation simply drops out of the candidate set; the
//! resolution degrades to `Unavailable`, never to a fabricated coordinate.

use std::net::IpAddr;

use crate::service::GeoRecord;

use super::{resolve_candidates, GeoCandidate, GeoResolution, ProviderRef, MAX_CANDIDATES};

/// Failures surfaced by the underlying provider adapter. The adapter may use
/// these for its own diagnostics; the orchestrator treats them all as "no
/// data from this provider".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderFailure {
    /// The provider endpoint is unreachable.
    Unreachable,
    /// The provider lookup exceeded its bounded deadline.
    Timeout,
    /// The provider returned syntactically plausible but unusable data.
    InvalidData,
}

/// Provider network boundary for Geo lookups.
///
/// Implementations talk to the chosen online Geo provider on the router. They
/// must not retain credentials or precise user locations beyond the record
/// itself.
pub trait GeoProviderRuntime {
    /// Resolve `ip` through `provider`.
    ///
    /// Returns the record together with the address it was actually resolved
    /// for, so the caller can reject data produced for a different exit.
    fn lookup(
        &mut self,
        provider: ProviderRef,
        ip: IpAddr,
    ) -> Result<Option<(IpAddr, GeoRecord)>, ProviderFailure>;
}

impl GeoProviderRuntime for Box<dyn GeoProviderRuntime> {
    fn lookup(
        &mut self,
        provider: ProviderRef,
        ip: IpAddr,
    ) -> Result<Option<(IpAddr, GeoRecord)>, ProviderFailure> {
        (**self).lookup(provider, ip)
    }
}

/// Query up to [`MAX_CANDIDATES`] providers and resolve the exit location.
///
/// Providers are consulted in order; failures, empty results, wrong-exit
/// records, and records that fail validation are dropped from the candidate
/// set. The result is `Fresh` only on a single (or agreeing) valid record for
/// the expected exit, `Uncertain` on conflicting valid records, and
/// `Unavailable` otherwise.
pub fn resolve_geo(
    runtime: &mut impl GeoProviderRuntime,
    expected_exit_ip: IpAddr,
    providers: &[ProviderRef],
    now_unix: u64,
) -> GeoResolution {
    let candidates: Vec<GeoCandidate> = providers
        .iter()
        .take(MAX_CANDIDATES)
        .filter_map(|provider| {
            runtime
                .lookup(provider.clone(), expected_exit_ip)
                .ok()
                .flatten()
                .map(|(resolved_ip, record)| {
                    GeoCandidate::new(provider.clone(), resolved_ip, record)
                })
        })
        .collect();
    eprintln!(
        "wloc resolve_geo: expected={expected_exit_ip} candidates={}",
        candidates.len()
    );
    for candidate in &candidates {
        if candidate.record.validate_at(now_unix).is_err() {
            eprintln!(
                "wloc resolve_geo: candidate rejected country={}",
                candidate.record.country_code
            );
        }
    }

    resolve_candidates(expected_exit_ip, &candidates, now_unix)
        .unwrap_or(GeoResolution::Unavailable)
}
