//! Safety contract for the standalone router-side WLOC service.
//!
//! This module deliberately contains no private WLOC protocol parser or
//! response mutation. It defines the scope and fail-open decisions that can be
//! implemented before authorized fixture and license gates are closed.

use std::net::IpAddr;
use std::time::Duration;

use crate::APPROVED_WLOC_HOSTS;

pub const SERVICE_API_VERSION: u16 = 1;
const MAX_CONNECTIONS: u16 = 32;
const MAX_FAILURE_GRACE: Duration = Duration::from_secs(30);

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
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
    pub expires_at_unix: u64,
}

impl GeoRecord {
    fn is_usable_at(&self, now_unix: u64) -> bool {
        let valid_country = self.country_code.len() == 2
            && self
                .country_code
                .bytes()
                .all(|value| value.is_ascii_uppercase());
        let valid_coordinates = self.latitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && self.longitude.is_finite()
            && (-180.0..=180.0).contains(&self.longitude);
        let valid_timezone = !self.timezone.is_empty()
            && self.timezone.len() <= 64
            && self.timezone.bytes().all(|value| {
                value.is_ascii_alphanumeric() || matches!(value, b'/' | b'_' | b'-' | b'+')
            });

        valid_country && valid_coordinates && valid_timezone && self.expires_at_unix > now_unix
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHealth {
    Healthy,
    Unhealthy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutingAction {
    Intercept,
    PassThrough,
    RemoveRedirect,
}

pub fn decide_routing(
    config: &ServiceConfig,
    traffic: &TrafficMeta,
    geo: Option<&GeoRecord>,
    health: RuntimeHealth,
    now_unix: u64,
) -> RoutingAction {
    if !config.enabled {
        return RoutingAction::PassThrough;
    }
    if config.validate().is_err() || health != RuntimeHealth::Healthy {
        return RoutingAction::RemoveRedirect;
    }
    if traffic.source_ip != config.assigned_device
        || traffic.transport != Transport::Tcp
        || traffic.destination_port != 443
        || !APPROVED_WLOC_HOSTS.contains(&traffic.hostname.as_str())
    {
        return RoutingAction::PassThrough;
    }
    if !geo.is_some_and(|record| record.is_usable_at(now_unix)) {
        return RoutingAction::PassThrough;
    }

    RoutingAction::Intercept
}
