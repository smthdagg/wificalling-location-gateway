//! Bounded v2 device profiles.
//!
//! A profile is the unit that will eventually connect one LAN device to one
//! node, one WLOC location policy, and one service lifecycle.  This module is
//! deliberately independent from runtime control: validation happens before
//! any UCI write, nftables operation, or child-process change.

use std::fmt;
use std::net::IpAddr;

use serde::Serialize;

use super::uci::{LocationMode, WlocUciConfig};

pub const MAX_PROFILES: usize = 8;
pub const MAX_PROFILE_ID_BYTES: usize = 32;
pub const MAX_PROFILE_LABEL_BYTES: usize = 48;
pub const MAX_NODE_REF_BYTES: usize = 96;
pub const MAX_LOCATION_REF_BYTES: usize = 64;
pub const MAX_SERIALIZED_PROFILES_BYTES: usize = 8 * 1024;
pub const MAX_REDACTED_STATUS_BYTES: usize = 4 * 1024;
pub const MAX_UCI_TEXT_BYTES: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeSelectionMode {
    Fixed,
    GatewayDefault,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DeviceProfile {
    pub id: String,
    pub label: String,
    pub assigned_device: Option<String>,
    pub node_ref: String,
    pub node_mode: NodeSelectionMode,
    pub location_mode: LocationMode,
    pub manual_latitude: Option<f64>,
    pub manual_longitude: Option<f64>,
    pub manual_location_ref: Option<String>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileError {
    TooManyProfiles,
    DuplicateId(String),
    DuplicateAssignedDevice(String),
    MissingAssignedDevice,
    MultipleProfilesRequireUnifiedRuntime,
    EmptyField(&'static str),
    FieldTooLong { field: &'static str, max: usize },
    InvalidProfileId(String),
    InvalidDeviceAddress(String),
    InvalidNodeRef,
    IncompleteLocation,
    InvalidLocation,
    SerializedTooLarge,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyProfiles => write!(formatter, "too many device profiles"),
            Self::DuplicateId(id) => write!(formatter, "duplicate device profile id: {id}"),
            Self::DuplicateAssignedDevice(address) => {
                write!(
                    formatter,
                    "device is assigned to multiple profiles: {address}"
                )
            }
            Self::MissingAssignedDevice => {
                formatter.write_str("assigned device address is required")
            }
            Self::MultipleProfilesRequireUnifiedRuntime => {
                formatter.write_str("multiple profiles require the unified runtime")
            }
            Self::EmptyField(field) => write!(formatter, "profile field is empty: {field}"),
            Self::FieldTooLong { field, max } => {
                write!(
                    formatter,
                    "profile field is too long: {field} (max {max} bytes)"
                )
            }
            Self::InvalidProfileId(id) => write!(formatter, "invalid device profile id: {id}"),
            Self::InvalidDeviceAddress(address) => {
                write!(formatter, "invalid assigned device address: {address}")
            }
            Self::InvalidNodeRef => formatter.write_str("invalid node reference"),
            Self::IncompleteLocation => formatter.write_str("manual location is incomplete"),
            Self::InvalidLocation => formatter.write_str("manual location is invalid"),
            Self::SerializedTooLarge => formatter.write_str("device profiles exceed storage bound"),
        }
    }
}

impl std::error::Error for ProfileError {}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileModel {
    profiles: Vec<DeviceProfile>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeProfile {
    pub id: String,
    pub enabled: bool,
    pub runtime_supported: bool,
    pub assigned_device: Option<String>,
    pub node_ref: String,
    pub location_mode: LocationMode,
    pub manual_latitude: Option<f64>,
    pub manual_longitude: Option<f64>,
}

#[derive(Serialize)]
struct RedactedProfileStatus<'a> {
    profile_id: &'a str,
    label: &'a str,
    enabled: bool,
    node_mode: NodeSelectionMode,
    location_mode: LocationMode,
    assigned_device_configured: bool,
    manual_location_configured: bool,
}

impl ProfileModel {
    pub fn new(profiles: Vec<DeviceProfile>) -> Result<Self, ProfileError> {
        let model = Self { profiles };
        model.validate()?;
        Ok(model)
    }

    /// Convert the v1 singleton configuration without changing its effective
    /// behavior. Re-running this function produces the same model.
    pub fn from_legacy(config: &WlocUciConfig) -> Result<Self, ProfileError> {
        let has_manual_coordinates =
            config.manual_latitude.is_some() || config.manual_longitude.is_some();
        Self::new(vec![DeviceProfile {
            id: "default".to_owned(),
            label: "Default device".to_owned(),
            assigned_device: (!config.assigned_device.trim().is_empty())
                .then(|| config.assigned_device.clone()),
            node_ref: config.node_ref.clone(),
            node_mode: NodeSelectionMode::Fixed,
            location_mode: config.location_mode,
            manual_latitude: config.manual_latitude,
            manual_longitude: config.manual_longitude,
            manual_location_ref: has_manual_coordinates.then(|| "legacy-manual".to_owned()),
            enabled: config.enabled,
        }])
    }

    pub fn profiles(&self) -> &[DeviceProfile] {
        &self.profiles
    }

    /// The current daemon is still a single-runtime process. It may consume
    /// exactly one profile, but must refuse to guess when multiple profiles
    /// are configured before unified multi-device routing lands.
    pub fn single_runtime_profile(&self) -> Result<RuntimeProfile, ProfileError> {
        let profile = self
            .profiles
            .first()
            .filter(|_| self.profiles.len() == 1)
            .ok_or(ProfileError::MultipleProfilesRequireUnifiedRuntime)?;
        Ok(RuntimeProfile {
            id: profile.id.clone(),
            enabled: profile.enabled,
            runtime_supported: profile
                .assigned_device
                .as_deref()
                .map(|address| address.parse::<IpAddr>().is_ok())
                .unwrap_or(true),
            assigned_device: profile.assigned_device.clone(),
            node_ref: profile.node_ref.clone(),
            location_mode: profile.location_mode,
            manual_latitude: profile.manual_latitude,
            manual_longitude: profile.manual_longitude,
        })
    }

    /// Validate a candidate before replacing the current model. The existing
    /// value is left untouched on every error.
    pub fn replace(&mut self, profiles: Vec<DeviceProfile>) -> Result<(), ProfileError> {
        let candidate = Self::new(profiles)?;
        self.profiles = candidate.profiles;
        Ok(())
    }

    pub fn redacted_status(&self) -> Result<Vec<serde_json::Value>, ProfileError> {
        let status: Vec<_> = self
            .profiles
            .iter()
            .map(|profile| {
                serde_json::to_value(RedactedProfileStatus {
                    profile_id: &profile.id,
                    label: &profile.label,
                    enabled: profile.enabled,
                    node_mode: profile.node_mode,
                    location_mode: profile.location_mode,
                    assigned_device_configured: profile.assigned_device.is_some(),
                    manual_location_configured: profile.manual_latitude.is_some()
                        && profile.manual_longitude.is_some(),
                })
                .map_err(|_| ProfileError::SerializedTooLarge)
            })
            .collect::<Result<_, _>>()?;
        let encoded = serde_json::to_vec(&status).map_err(|_| ProfileError::SerializedTooLarge)?;
        if encoded.len() > MAX_REDACTED_STATUS_BYTES {
            return Err(ProfileError::SerializedTooLarge);
        }
        Ok(status)
    }

    fn validate(&self) -> Result<(), ProfileError> {
        if self.profiles.len() > MAX_PROFILES {
            return Err(ProfileError::TooManyProfiles);
        }
        let mut ids = Vec::with_capacity(self.profiles.len());
        let mut assigned_devices = Vec::with_capacity(self.profiles.len());
        for profile in &self.profiles {
            validate_profile_id(&profile.id)?;
            if !ids.iter().any(|id: &String| id == &profile.id) {
                ids.push(profile.id.clone());
            } else {
                return Err(ProfileError::DuplicateId(profile.id.clone()));
            }
            validate_bounded_text("label", &profile.label, MAX_PROFILE_LABEL_BYTES)?;
            if let Some(address) = profile.assigned_device.as_deref() {
                validate_device_address(address)?;
                let normalized = normalize_device_address(address);
                if assigned_devices.contains(&normalized) {
                    return Err(ProfileError::DuplicateAssignedDevice(address.to_owned()));
                }
                assigned_devices.push(normalized);
            }
            validate_node_ref(&profile.node_ref)?;
            if let Some(reference) = profile.manual_location_ref.as_deref() {
                validate_location_ref(reference)?;
            }
            let has_latitude = profile.manual_latitude.is_some();
            let has_longitude = profile.manual_longitude.is_some();
            if has_latitude != has_longitude {
                return Err(ProfileError::IncompleteLocation);
            }
            if let (Some(latitude), Some(longitude)) =
                (profile.manual_latitude, profile.manual_longitude)
            {
                if !latitude.is_finite()
                    || !longitude.is_finite()
                    || !(-90.0..=90.0).contains(&latitude)
                    || !(-180.0..=180.0).contains(&longitude)
                {
                    return Err(ProfileError::InvalidLocation);
                }
            }
            if profile.location_mode == LocationMode::Manual && !(has_latitude && has_longitude) {
                return Err(ProfileError::IncompleteLocation);
            }
        }
        let encoded =
            serde_json::to_vec(&self.profiles).map_err(|_| ProfileError::SerializedTooLarge)?;
        if encoded.len() > MAX_SERIALIZED_PROFILES_BYTES {
            return Err(ProfileError::SerializedTooLarge);
        }
        Ok(())
    }
}

pub fn validate_profile_id(value: &str) -> Result<(), ProfileError> {
    if value.is_empty() {
        return Err(ProfileError::EmptyField("id"));
    }
    if value.len() > MAX_PROFILE_ID_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(ProfileError::InvalidProfileId(value.to_owned()));
    }
    Ok(())
}

pub fn validate_device_address(value: &str) -> Result<(), ProfileError> {
    if !is_valid_device_address(value) {
        return Err(ProfileError::InvalidDeviceAddress(value.to_owned()));
    }
    Ok(())
}

pub fn validate_node_ref(value: &str) -> Result<(), ProfileError> {
    if value.is_empty()
        || value.len() > MAX_NODE_REF_BYTES
        || value.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || character == '/'
                || character == '\\'
        })
    {
        return Err(ProfileError::InvalidNodeRef);
    }
    Ok(())
}

pub fn validate_profile_label(value: &str) -> Result<(), ProfileError> {
    validate_bounded_text("label", value, MAX_PROFILE_LABEL_BYTES)
}

pub fn validate_location_ref(value: &str) -> Result<(), ProfileError> {
    validate_bounded_text("manual_location_ref", value, MAX_LOCATION_REF_BYTES)
}

fn validate_bounded_text(field: &'static str, value: &str, max: usize) -> Result<(), ProfileError> {
    if value.is_empty() {
        return Err(ProfileError::EmptyField(field));
    }
    if value.len() > max {
        return Err(ProfileError::FieldTooLong { field, max });
    }
    Ok(())
}

fn is_valid_device_address(value: &str) -> bool {
    if let Ok(address) = value.parse::<IpAddr>() {
        return match address {
            IpAddr::V4(ip) => {
                let octets = ip.octets();
                !(ip.is_unspecified()
                    || ip.is_loopback()
                    || ip.is_multicast()
                    || ip == std::net::Ipv4Addr::BROADCAST
                    || (octets[0] == 169 && octets[1] == 254))
                    && (octets[0] == 10
                        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                        || (octets[0] == 192 && octets[1] == 168))
            }
            IpAddr::V6(ip) => {
                let first = ip.segments()[0];
                !ip.is_unspecified()
                    && !ip.is_loopback()
                    && !ip.is_multicast()
                    && (first & 0xffc0) != 0xfe80
                    && (first & 0xfe00) == 0xfc00
            }
        };
    }
    is_valid_mac(value)
}

fn normalize_device_address(value: &str) -> String {
    if let Ok(address) = value.parse::<IpAddr>() {
        return address.to_string();
    }
    value
        .bytes()
        .filter(|byte| *byte != b':' && *byte != b'-')
        .map(|byte| byte.to_ascii_lowercase() as char)
        .collect()
}

fn is_valid_mac(value: &str) -> bool {
    let separator = if value.contains(':') { ':' } else { '-' };
    let parts: Vec<_> = value.split(separator).collect();
    let first = parts
        .first()
        .and_then(|part| u8::from_str_radix(part, 16).ok());
    parts.len() == 6
        && parts
            .iter()
            .all(|part| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_hexdigit()))
        && parts
            .iter()
            .any(|part| part.bytes().any(|byte| byte != b'0'))
        && first.is_some_and(|byte| byte & 1 == 0)
}
