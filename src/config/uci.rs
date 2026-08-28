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

pub const DEFAULT_UCI_PATH: &str = "/etc/config/wloc-service";
pub const DEFAULT_PROBE_INTERVAL_SECS: u64 = 300;
const DEFAULT_NODE_REF: &str = "default";

/// Where the location target comes from: follow the bound node's exit, or
/// use the manually configured coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    pub presets: Vec<Preset>,
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
            presets: Vec::new(),
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
        let text =
            std::fs::read_to_string(path).map_err(|_| UciError::Io(path.display().to_string()))?;
        Self::parse(&text)
    }

    /// Parse UCI text. A missing `main` section yields the defaults.
    pub fn parse(text: &str) -> Result<Self, UciError> {
        let mut config = Self::default();
        let mut section_type: Option<String> = None;
        let mut section_name: Option<String> = None;
        let mut preset: Option<PresetBuilder> = None;

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
                    match tokens.as_slice() {
                        [_keyword, type_name, name, ..] => {
                            section_type = Some(type_name.clone());
                            section_name = Some(name.clone());
                        }
                        [_keyword, type_name] => {
                            section_type = Some(type_name.clone());
                            section_name = None;
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
            _ => {}
        },
        _ => {}
    }
    Ok(())
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
