//! Safety contract for the standalone router-side WLOC service.
//!
//! This module deliberately contains no private WLOC protocol parser or
//! response mutation. It defines the scope and fail-open decisions that can be
//! implemented before authorized fixture and license gates are closed.

use std::net::IpAddr;
use std::time::Duration;

use crate::APPROVED_WLOC_HOSTS;

pub mod api;
pub mod control;
pub mod dispatch;
pub mod profile_runtime;
#[cfg(unix)]
pub mod server;
pub mod state;
pub mod status;
pub mod supervisor;

pub const SERVICE_API_VERSION: u16 = 1;
const MAX_CONNECTIONS: u16 = 32;
const MAX_FAILURE_GRACE: Duration = Duration::from_secs(30);
const MAX_GEO_TTL_SECONDS: u64 = 3_600;
/// Tolerated offset between the record's production time and the consumer's
/// clock. The sing-box exit probe and provider lookup can take several
/// seconds, so a freshly produced record may legitimately carry an expiry a
/// little beyond `MAX_GEO_TTL_SECONDS` when validated against an earlier
/// snapshot timestamp.
const MAX_GEO_TTL_GRACE_SECONDS: u64 = 300;
const ISO_3166_ALPHA2: &str = concat!(
    "ADAEAFAGAIALAMAOARAQASATAUAWAXAZBABBBDBEBFBGBHBIBJBLBMBNBOBQBRBSBTBVBWBYBZ",
    "CACCCDCFCGCHCICKCLCMCNCOCRCUCVCWCXCYCZDEDJDKDMDODZECEEEGEHERESETFIFJFKFMFOFR",
    "GAGBGDGEGFGGGHGIGLGMGNGPGQGRGSGTGUGWGYHKHMHNHRHTHUIDIEILIMINIOIQIRISIT",
    "JEJMJOJPKEKGKHKIKMKNKPKRKWKYKZLALBLCLILKLRLSLTLULVLYMAMCMDMEMFMGMHMKML",
    "MMMNMOMPMQMRMSMTMUMVMWMXMYMZNANCNENFNGNINLNONPNRNUNZOMPAPEPFPGPH",
    "PKPLPMPNPRPSPTPWPYQARERORSRURWSASBSCSDSESGSHSISJSKSLSMSNSOSRSSSTSV",
    "SXSYSZTCTDTFTGTHTJTKTLTMTNTOTRTTTVTWTZUAUGUMUSUYUZVAVCVEVGVIVNVUWF",
    "WSYEYTZAZMZW"
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceConfig {
    pub enabled: bool,
    pub assigned_device: IpAddr,
    pub max_connections: u16,
    pub failure_grace: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceConfigError {
    InvalidMaxConnections,
    InvalidFailureGrace,
}

impl ServiceConfig {
    pub fn validate(&self) -> Result<(), ServiceConfigError> {
        if self.max_connections == 0 || self.max_connections > MAX_CONNECTIONS {
            return Err(ServiceConfigError::InvalidMaxConnections);
        }
        if self.failure_grace.is_zero() || self.failure_grace > MAX_FAILURE_GRACE {
            return Err(ServiceConfigError::InvalidFailureGrace);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transport {
    Tcp,
    Udp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficMeta {
    pub source_ip: IpAddr,
    pub hostname: String,
    pub transport: Transport,
    pub destination_port: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoRecord {
    pub country_code: String,
    /// City/region name from the provider (UI display only; never in status).
    pub city: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
    pub expires_at_unix: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedGeo(GeoRecord);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeoValidationError {
    InvalidCountryCode,
    InvalidCoordinates,
    InvalidTimezone,
    Expired,
}

/// Validate a two-letter ISO 3166 alpha-2 country code.
pub fn is_valid_country_code(code: &str) -> bool {
    code.len() == 2
        && code.bytes().all(|value| value.is_ascii_uppercase())
        && ISO_3166_ALPHA2
            .as_bytes()
            .chunks_exact(2)
            .any(|candidate| candidate == code.as_bytes())
}

impl GeoRecord {
    pub fn validate_at(&self, now_unix: u64) -> Result<ValidatedGeo, GeoValidationError> {
        let valid_country = is_valid_country_code(&self.country_code);
        let valid_coordinates = self.latitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && self.longitude.is_finite()
            && (-180.0..=180.0).contains(&self.longitude);
        let valid_timezone = self.timezone == "UTC"
            || (self.timezone.contains('/')
                && self.timezone.len() <= 64
                && self.timezone.split('/').all(|segment| !segment.is_empty())
                && self.timezone.bytes().all(|value| {
                    value.is_ascii_alphanumeric() || matches!(value, b'/' | b'_' | b'-' | b'+')
                }));

        if !valid_country {
            return Err(GeoValidationError::InvalidCountryCode);
        }
        if !valid_coordinates {
            return Err(GeoValidationError::InvalidCoordinates);
        }
        if !valid_timezone {
            return Err(GeoValidationError::InvalidTimezone);
        }
        if self.expires_at_unix <= now_unix
            || self.expires_at_unix.saturating_sub(now_unix)
                > MAX_GEO_TTL_SECONDS + MAX_GEO_TTL_GRACE_SECONDS
        {
            return Err(GeoValidationError::Expired);
        }
        Ok(ValidatedGeo(self.clone()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealth {
    Healthy,
    Unhealthy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngressDisposition {
    RouteToMitm,
    BypassMitm,
    WithdrawRedirect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseMode {
    ForwardOriginal,
    PatchAuthorized,
}

/// Private-protocol handling remains gated, so the current service can only
/// forward an upstream response unchanged.
pub const fn current_response_mode() -> ResponseMode {
    ResponseMode::ForwardOriginal
}

pub fn decide_ingress(
    config: &ServiceConfig,
    traffic: &TrafficMeta,
    health: RuntimeHealth,
) -> IngressDisposition {
    if !config.enabled {
        return IngressDisposition::BypassMitm;
    }
    if config.validate().is_err() || health != RuntimeHealth::Healthy {
        return IngressDisposition::WithdrawRedirect;
    }
    if traffic.source_ip != config.assigned_device
        || traffic.transport != Transport::Tcp
        || traffic.destination_port != 443
        || !APPROVED_WLOC_HOSTS.contains(&traffic.hostname.as_str())
    {
        return IngressDisposition::BypassMitm;
    }

    IngressDisposition::RouteToMitm
}
