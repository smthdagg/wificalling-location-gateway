//! Minimal UCI parser for `/etc/config/wloc-service`.
//!
//! UCI files are shell-like: `config <type> [name]` sections with
//! `option <name> <value>` lines. Values may be bare tokens or single-quoted.
//! Comments start with `#`. Only the sections the daemon cares about are
//! surfaced; unknown sections and options are ignored so LuCI can extend the
//! file without breaking the daemon.

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::Serialize;

pub const DEFAULT_UCI_PATH: &str = "/etc/config/wloc-service";
pub const DEFAULT_PROBE_PORT: u16 = 18080;
pub const DEFAULT_PROBE_INTERVAL_SECS: u64 = 300;
const DEFAULT_NODE_REF: &str = "default";

/// Where the location target comes from: follow the bound node's exit, or
/// use the manually configured coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationMode {
    Auto,
    Manual,
}

/// A saved manual-location preset (`config preset` section).
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub label: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// Parsed daemon configuration with defaults for every absent option.
#[derive(Clone, Debug, PartialEq)]
pub struct WlocUciConfig {
    pub enabled: bool,
    pub location_mode: LocationMode,
    pub manual_latitude: Option<f64>,
    pub manual_longitude: Option<f64>,
    pub node_ref: String,
    pub assigned_device: String,
    pub probe_interval_secs: u64,
    pub geo_provider: String,
    pub probe_port: u16,
    pub singbox_config: String,
    pub presets: Vec<Preset>,
    /// Explicit v2 device sections. An empty list means the legacy singleton
    /// fields above are still the source and can be migrated by `profile_model`.
    pub profiles: Vec<super::profile::DeviceProfile>,
}

impl Default for WlocUciConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            location_mode: LocationMode::Auto,
            manual_latitude: None,
            manual_longitude: None,
            node_ref: DEFAULT_NODE_REF.to_owned(),
            assigned_device: String::new(),
            probe_interval_secs: DEFAULT_PROBE_INTERVAL_SECS,
            geo_provider: "http".to_owned(),
            probe_port: DEFAULT_PROBE_PORT,
            singbox_config: "/var/run/wloc-service/sing-box.json".to_owned(),
            presets: Vec::new(),
            profiles: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum UciError {
    /// The file could not be read.
    Io(String),
    /// A line does not follow the `config`/`option` grammar.
    Syntax { line: usize, text: String },
    /// A coordinate value is not a valid f64.
    Coordinate { option: String, value: String },
    /// A device profile section failed v2 validation.
    Profile(String),
    /// The complete UCI input exceeds the small-gateway parser bound.
    ConfigTooLarge,
}

impl fmt::Display for UciError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UciError::Io(path) => write!(f, "cannot read UCI file {path}"),
            UciError::Syntax { line, text } => {
                write!(f, "malformed UCI line {line}: {text}")
            }
            UciError::Coordinate { option, value } => {
                write!(f, "invalid {option} coordinate: {value}")
            }
            UciError::Profile(message) => write!(f, "invalid device profile: {message}"),
            UciError::ConfigTooLarge => write!(f, "UCI configuration exceeds the size limit"),
        }
    }
}

impl std::error::Error for UciError {}

/// Accumulates one `config preset` section while its options stream in.
#[derive(Default)]
struct PresetBuilder {
    name: String,
    label: String,
    latitude: f64,
    longitude: f64,
}

impl PresetBuilder {
    fn finish(self) -> Preset {
        Preset {
            name: self.name,
            label: self.label,
            latitude: self.latitude,
            longitude: self.longitude,
        }
    }
}

impl WlocUciConfig {
    /// Read and parse the daemon configuration file.
    pub fn load(path: &Path) -> Result<Self, UciError> {
        let metadata =
            std::fs::metadata(path).map_err(|_| UciError::Io(path.display().to_string()))?;
        if metadata.len() > super::profile::MAX_UCI_TEXT_BYTES as u64 {
            return Err(UciError::ConfigTooLarge);
        }
        let text =
            std::fs::read_to_string(path).map_err(|_| UciError::Io(path.display().to_string()))?;
        Self::parse(&text)
    }

    /// Return the explicit v2 profiles, or a deterministic singleton model
    /// synthesized from the v1 fields when no device sections exist.
    pub fn profile_model(
        &self,
    ) -> Result<super::profile::ProfileModel, super::profile::ProfileError> {
        if self.profiles.is_empty() {
            super::profile::ProfileModel::from_legacy(self)
        } else {
            super::profile::ProfileModel::new(self.profiles.clone())
        }
    }

    /// Parse UCI text. A missing `main` section yields the defaults.
    pub fn parse(text: &str) -> Result<Self, UciError> {
        if text.len() > super::profile::MAX_UCI_TEXT_BYTES {
            return Err(UciError::ConfigTooLarge);
        }
        let mut config = Self::default();
        let mut section_type: Option<String> = None;
        let mut section_name: Option<String> = None;
        let mut preset: Option<PresetBuilder> = None;
        let mut device: Option<DeviceBuilder> = None;

        for (index, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let tokens = split_uci_tokens(line).ok_or_else(|| UciError::Syntax {
                line: index + 1,
                text: line.to_owned(),
            })?;
            match tokens.first().map(String::as_str) {
                Some("config") => {
                    if let Some(builder) = preset.take() {
                        config.presets.push(builder.finish());
                    }
                    if let Some(builder) = device.take() {
                        config
                            .profiles
                            .push(builder.finish().map_err(profile_error)?);
                    }
                    match tokens.as_slice() {
                        [_keyword, type_name, name, ..] => {
                            section_type = Some(type_name.clone());
                            section_name = Some(name.clone());
                            if type_name == "device" {
                                device = Some(DeviceBuilder::new(name.clone()));
                            }
                        }
                        [_keyword, type_name] => {
                            section_type = Some(type_name.clone());
                            section_name = None;
                            if type_name == "device" {
                                device = Some(DeviceBuilder::new("default".to_owned()));
                            }
                        }
                        _ => {
                            return Err(UciError::Syntax {
                                line: index + 1,
                                text: line.to_owned(),
                            })
                        }
                    }
                }
                Some("option") => {
                    let (name, value) = match tokens.as_slice() {
                        [_option, name, value] => (name.as_str(), value.as_str()),
                        _ => {
                            return Err(UciError::Syntax {
                                line: index + 1,
                                text: line.to_owned(),
                            })
                        }
                    };
                    apply_option(
                        &mut config,
                        &mut preset,
                        &mut device,
                        section_type.as_deref(),
                        section_name.as_deref(),
                        name,
                        value,
                    )?;
                }
                Some(_) => {
                    return Err(UciError::Syntax {
                        line: index + 1,
                        text: line.to_owned(),
                    })
                }
                None => {}
            }
        }
        if let Some(builder) = preset.take() {
            config.presets.push(builder.finish());
        }
        if let Some(builder) = device.take() {
            config
                .profiles
                .push(builder.finish().map_err(profile_error)?);
        }
        if !config.profiles.is_empty() {
            super::profile::ProfileModel::new(config.profiles.clone()).map_err(profile_error)?;
        }
        Ok(config)
    }
}

/// Split a UCI line into tokens, honoring single-quoted values.
fn split_uci_tokens(line: &str) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    for ch in line.chars() {
        if ch == '\'' {
            if in_quote {
                in_quote = false;
                tokens.push(std::mem::take(&mut current));
            } else {
                in_quote = true;
            }
        } else if ch.is_whitespace() && !in_quote {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if in_quote {
        return None; // unterminated quote
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Some(tokens)
}

fn apply_option(
    config: &mut WlocUciConfig,
    preset: &mut Option<PresetBuilder>,
    device: &mut Option<DeviceBuilder>,
    section_type: Option<&str>,
    section_name: Option<&str>,
    name: &str,
    value: &str,
) -> Result<(), UciError> {
    match section_type {
        Some("preset") => {
            let builder = preset.get_or_insert_with(|| PresetBuilder {
                name: section_name.unwrap_or_default().to_owned(),
                ..PresetBuilder::default()
            });
            match name {
                "label" => builder.label = value.to_owned(),
                "latitude" => builder.latitude = parse_coord(name, value)?,
                "longitude" => builder.longitude = parse_coord(name, value)?,
                _ => {}
            }
        }
        Some("device") => {
            let builder = device.as_mut().ok_or_else(|| {
                UciError::Profile("device option outside a device section".to_owned())
            })?;
            match name {
                "label" | "name" => builder.label = value.to_owned(),
                "assigned_device" => builder.assigned_device = Some(value.to_owned()),
                "node_ref" => builder.node_ref = value.to_owned(),
                "node_mode" => {
                    builder.node_mode = match value {
                        "fixed" => super::profile::NodeSelectionMode::Fixed,
                        _ => return Err(UciError::Profile("unknown node_mode".to_owned())),
                    }
                }
                "geo_source" => {
                    builder.location_mode = match value {
                        "auto" => LocationMode::Auto,
                        "manual" => LocationMode::Manual,
                        _ => return Err(UciError::Profile("unknown geo_source".to_owned())),
                    }
                }
                "manual_lat" => builder.manual_latitude = Some(parse_coord(name, value)?),
                "manual_lon" => builder.manual_longitude = Some(parse_coord(name, value)?),
                "manual_location_ref" => builder.manual_location_ref = Some(value.to_owned()),
                "enabled" => builder.enabled = matches!(value, "1" | "true" | "on"),
                _ => {}
            }
        }
        Some("wloc-service") if section_name == Some("main") => match name {
            "enabled" => config.enabled = matches!(value, "1" | "true" | "on"),
            "geo_source" => {
                config.location_mode = if value == "manual" {
                    LocationMode::Manual
                } else {
                    LocationMode::Auto
                }
            }
            "manual_lat" => config.manual_latitude = Some(parse_coord(name, value)?),
            "manual_lon" => config.manual_longitude = Some(parse_coord(name, value)?),
            "node_ref" => config.node_ref = value.to_owned(),
            "assigned_device" => config.assigned_device = value.to_owned(),
            "probe_interval" => {
                config.probe_interval_secs =
                    u64::from_str(value).unwrap_or(DEFAULT_PROBE_INTERVAL_SECS)
            }
            "geo_provider" => config.geo_provider = value.to_owned(),
            "probe_port" => config.probe_port = u16::from_str(value).unwrap_or(DEFAULT_PROBE_PORT),
            "singbox_config" => config.singbox_config = value.to_owned(),
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

fn profile_error(error: super::profile::ProfileError) -> UciError {
    UciError::Profile(error.to_string())
}

struct DeviceBuilder {
    id: String,
    label: String,
    assigned_device: Option<String>,
    node_ref: String,
    node_mode: super::profile::NodeSelectionMode,
    location_mode: LocationMode,
    manual_latitude: Option<f64>,
    manual_longitude: Option<f64>,
    manual_location_ref: Option<String>,
    enabled: bool,
}

impl DeviceBuilder {
    fn new(id: String) -> Self {
        Self {
            id,
            label: "Default device".to_owned(),
            assigned_device: None,
            node_ref: DEFAULT_NODE_REF.to_owned(),
            node_mode: super::profile::NodeSelectionMode::Fixed,
            location_mode: LocationMode::Auto,
            manual_latitude: None,
            manual_longitude: None,
            manual_location_ref: None,
            enabled: true,
        }
    }

    fn finish(self) -> Result<super::profile::DeviceProfile, super::profile::ProfileError> {
        if self.assigned_device.is_none() {
            return Err(super::profile::ProfileError::MissingAssignedDevice);
        }
        let profile = super::profile::DeviceProfile {
            id: self.id,
            label: self.label,
            assigned_device: self.assigned_device,
            node_ref: self.node_ref,
            node_mode: self.node_mode,
            location_mode: self.location_mode,
            manual_latitude: self.manual_latitude,
            manual_longitude: self.manual_longitude,
            manual_location_ref: self.manual_location_ref,
            enabled: self.enabled,
        };
        super::profile::ProfileModel::new(vec![profile.clone()]).map(|_| profile)
    }
}

fn parse_coord(option: &str, value: &str) -> Result<f64, UciError> {
    f64::from_str(value).map_err(|_| UciError::Coordinate {
        option: option.to_owned(),
        value: value.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_configuration() {
        let text = r#"
# WLOC service configuration
config wloc-service 'main'
	option enabled '1'
	option geo_source 'manual'
	option manual_lat '51.5074'
	option manual_lon '-0.1278'
	option node_ref 'uk_anytls_test'
	option assigned_device '192.168.1.100'
	option probe_interval '600'
	option geo_provider 'http'
	option probe_port '18080'
"#;
        let config = WlocUciConfig::parse(text).unwrap();
        assert!(config.enabled);
        assert_eq!(config.location_mode, LocationMode::Manual);
        assert_eq!(config.manual_latitude, Some(51.5074));
        assert_eq!(config.manual_longitude, Some(-0.1278));
        assert_eq!(config.node_ref, "uk_anytls_test");
        assert_eq!(config.assigned_device, "192.168.1.100");
        assert_eq!(config.probe_interval_secs, 600);
        assert_eq!(config.geo_provider, "http");
        assert_eq!(config.probe_port, 18080);
    }

    #[test]
    fn missing_main_section_uses_defaults() {
        let config = WlocUciConfig::parse("# nothing here\n").unwrap();
        assert_eq!(config, WlocUciConfig::default());
    }

    #[test]
    fn disabled_and_auto_mode() {
        let text = "config wloc-service 'main'\n\toption enabled '0'\n";
        let config = WlocUciConfig::parse(text).unwrap();
        assert!(!config.enabled);
        assert_eq!(config.location_mode, LocationMode::Auto);
        assert_eq!(config.manual_latitude, None);
    }

    #[test]
    fn invalid_coordinate_rejected() {
        let text = "config wloc-service 'main'\n\toption manual_lat 'abc'\n";
        assert!(matches!(
            WlocUciConfig::parse(text),
            Err(UciError::Coordinate { .. })
        ));
    }

    #[test]
    fn unterminated_quote_is_syntax_error() {
        let text = "config wloc-service 'main\n";
        assert!(matches!(
            WlocUciConfig::parse(text),
            Err(UciError::Syntax { .. })
        ));
    }

    #[test]
    fn presets_parsed_with_labels_and_coordinates() {
        let text = r#"
config wloc-service 'main'
	option enabled '1'
config preset 'hong_kong'
	option label '香港'
	option latitude '22.3193'
	option longitude '114.1694'
config preset 'london'
	option label 'London'
	option latitude '51.5074'
	option longitude '-0.1278'
"#;
        let config = WlocUciConfig::parse(text).unwrap();
        assert!(config.enabled);
        assert_eq!(config.presets.len(), 2);
        assert_eq!(config.presets[0].name, "hong_kong");
        assert_eq!(config.presets[0].label, "香港");
        assert_eq!(config.presets[0].latitude, 22.3193);
        assert_eq!(config.presets[1].name, "london");
        assert_eq!(config.presets[1].longitude, -0.1278);
    }

    #[test]
    fn load_missing_file_is_io_error() {
        assert!(matches!(
            WlocUciConfig::load(Path::new("/nonexistent/wloc-service")),
            Err(UciError::Io(_))
        ));
    }

    #[test]
    fn bare_unquoted_values_parse() {
        let text = "config wloc-service main\n\toption enabled 1\n\toption geo_source auto\n";
        let config = WlocUciConfig::parse(text).unwrap();
        assert!(config.enabled);
        assert_eq!(config.location_mode, LocationMode::Auto);
    }

    #[test]
    fn split_tokens_handles_quotes_and_spaces() {
        assert_eq!(
            split_uci_tokens("option label 'Hong Kong SAR'").unwrap(),
            vec![
                "option".to_owned(),
                "label".to_owned(),
                "Hong Kong SAR".to_owned()
            ]
        );
        assert!(split_uci_tokens("option label 'unterminated").is_none());
    }

    #[test]
    fn anonymous_section_ignored() {
        let text = "config wloc-service\n\toption enabled '1'\n";
        let config = WlocUciConfig::parse(text).unwrap();
        // An anonymous section is not `main`; defaults remain.
        assert!(config.enabled);
        assert_eq!(config.geo_provider, "http");
    }
}
