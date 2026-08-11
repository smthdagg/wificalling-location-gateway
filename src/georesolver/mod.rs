//! Deterministic Geo provider and cache policy.
//!
//! Provider network adapters are intentionally separate. Invalid, conflicting,
//! expired, or wrong-exit data never creates a fallback coordinate.

use std::net::IpAddr;

use crate::service::GeoRecord;

const MAX_PROVIDER_REF_BYTES: usize = 32;
const MAX_CANDIDATES: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRef(String);

impl ProviderRef {
    pub fn new(value: &str) -> Result<Self, GeoResolverError> {
        if value.is_empty()
            || value.len() > MAX_PROVIDER_REF_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(GeoResolverError::InvalidProviderRef);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoCandidate {
    provider: ProviderRef,
    exit_ip: IpAddr,
    record: GeoRecord,
}

impl GeoCandidate {
    pub const fn new(provider: ProviderRef, exit_ip: IpAddr, record: GeoRecord) -> Self {
        Self {
            provider,
            exit_ip,
            record,
        }
    }

    pub fn provider(&self) -> &ProviderRef {
        &self.provider
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoCacheEntry {
    exit_ip: IpAddr,
    record: GeoRecord,
}

impl GeoCacheEntry {
    pub const fn new(exit_ip: IpAddr, record: GeoRecord) -> Self {
        Self { exit_ip, record }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GeoResolution {
    Fresh(GeoRecord),
    Uncertain,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeoResolverError {
    InvalidProviderRef,
    TooManyCandidates,
}

pub fn resolve_candidates(
    expected_exit_ip: IpAddr,
    candidates: &[GeoCandidate],
    now_unix: u64,
) -> Result<GeoResolution, GeoResolverError> {
    if candidates.len() > MAX_CANDIDATES {
        return Err(GeoResolverError::TooManyCandidates);
    }

    let usable: Vec<&GeoRecord> = candidates
        .iter()
        .filter(|candidate| candidate.exit_ip == expected_exit_ip)
        .filter_map(|candidate| {
            candidate
                .record
                .validate_at(now_unix)
                .ok()
                .map(|_| &candidate.record)
        })
        .collect();

    Ok(match usable.as_slice() {
        [] => GeoResolution::Unavailable,
        [record] => GeoResolution::Fresh((*record).clone()),
        [first, second] if first == second => GeoResolution::Fresh((*first).clone()),
        [_, _] => GeoResolution::Uncertain,
        _ => unreachable!("candidate count is bounded before filtering"),
    })
}

pub fn select_cached(
    cache: &GeoCacheEntry,
    expected_exit_ip: IpAddr,
    now_unix: u64,
) -> Option<GeoRecord> {
    if cache.exit_ip != expected_exit_ip || cache.record.validate_at(now_unix).is_err() {
        return None;
    }
    Some(cache.record.clone())
}
