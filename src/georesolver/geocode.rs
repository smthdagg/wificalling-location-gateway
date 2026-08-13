//! Online place-name geocoding for manual location presets.
//!
//! Uses Nominatim (OpenStreetMap) to turn a place query into WGS84
//! coordinates. The endpoint is configurable so tests can run against a local
//! HTTP mock; production defaults to `https://nominatim.openstreetmap.org`.
//! No key is required, but the endpoint's usage policy expects a real
//! User-Agent and low request rates.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

/// Failures from geocoding a place query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeocodeError {
    /// The place could not be resolved to coordinates.
    NotFound,
    /// The provider returned malformed data.
    InvalidData,
    /// The provider could not be reached.
    Unreachable,
}

impl std::fmt::Display for GeocodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("place not found"),
            Self::InvalidData => formatter.write_str("geocoder returned invalid data"),
            Self::Unreachable => formatter.write_str("geocoder unreachable"),
        }
    }
}

impl std::error::Error for GeocodeError {}

const NOMINATIM_HOST: &str = "nominatim.openstreetmap.org";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_RESPONSE_BYTES: usize = 16 * 1024;

/// URL-encode a query for the `q` parameter (keeps unreserved ASCII bytes).
fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Geocode `query` against the default Nominatim endpoint.
pub fn geocode(query: &str) -> Result<(f64, f64), GeocodeError> {
    geocode_at(NOMINATIM_HOST, 443, query)
}

/// A geocoding result with the display name of the matched place.
#[derive(Clone, Debug, PartialEq)]
pub struct GeoSearchResult {
    pub city: String,
    pub latitude: f64,
    pub longitude: f64,
}

/// Geocode `query` and also return the matched place name (Nominatim
/// `display_name`), for the "search first, apply later" manual flow.
pub fn geocode_with_name(query: &str) -> Result<GeoSearchResult, GeocodeError> {
    geocode_with_name_at(NOMINATIM_HOST, 443, query)
}

/// Geocode `query` against `host:port`, returning name and coordinates.
pub fn geocode_with_name_at(
    host: &str,
    port: u16,
    query: &str,
) -> Result<GeoSearchResult, GeocodeError> {
    let path = format!(
        "/search?q={}&format=json&limit=1&accept-language=en",
        urlencode(query)
    );
    let body = if port == 443 {
        https_get(host, &path)?
    } else {
        http_get(host, port, &path)?
    };
    parse_geocode_response_with_name(&body)
}

/// Parse a Nominatim `format=json` response: an array whose first object
/// carries string `lat` and `lon` fields.
pub fn parse_geocode_response(body: &[u8]) -> Result<(f64, f64), GeocodeError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| GeocodeError::InvalidData)?;
    let first = value
        .as_array()
        .and_then(|items| items.first())
        .ok_or(GeocodeError::NotFound)?;
    let (latitude, longitude) = parse_lat_lon(first)?;
    Ok((latitude, longitude))
}

/// Like [`parse_geocode_response`], but also extracts the `display_name`.
pub fn parse_geocode_response_with_name(body: &[u8]) -> Result<GeoSearchResult, GeocodeError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| GeocodeError::InvalidData)?;
    let first = value
        .as_array()
        .and_then(|items| items.first())
        .ok_or(GeocodeError::NotFound)?;
    let (latitude, longitude) = parse_lat_lon(first)?;
    let city = first
        .get("display_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    Ok(GeoSearchResult {
        city,
        latitude,
        longitude,
    })
}

/// Reverse-geocode result: city/place name, ISO 3166-1 alpha-2 country code
/// and IANA timezone for a coordinate pair.
#[derive(Clone, Debug, PartialEq)]
pub struct ReverseGeoResult {
    pub city: String,
    pub country_code: String,
    pub timezone: String,
}

/// Reverse geocode `latitude, longitude` against the default Nominatim
/// endpoint, so a manual location can also show country/city/timezone.
pub fn reverse_geocode(latitude: f64, longitude: f64) -> Result<ReverseGeoResult, GeocodeError> {
    reverse_geocode_at(NOMINATIM_HOST, 443, latitude, longitude)
}

/// Reverse geocode against `host:port` (443 = TLS, 80 = plain HTTP).
pub fn reverse_geocode_at(
    host: &str,
    port: u16,
    latitude: f64,
    longitude: f64,
) -> Result<ReverseGeoResult, GeocodeError> {
    let path = format!("/reverse?lat={latitude}&lon={longitude}&format=json&accept-language=en");
    let body = if port == 443 {
        https_get(host, &path)?
    } else {
        http_get(host, port, &path)?
    };
    parse_reverse_geocode_response(&body)
}

/// Parse a Nominatim `/reverse` response: `display_name`, `address` with
/// `country_code`, and a `timezone` field.
pub fn parse_reverse_geocode_response(body: &[u8]) -> Result<ReverseGeoResult, GeocodeError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| GeocodeError::InvalidData)?;
    let display_name = value
        .get("display_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    if display_name.is_empty() {
        return Err(GeocodeError::NotFound);
    }
    let address = value.get("address").cloned().unwrap_or(Value::Null);
    let city = ["city", "town", "village", "county", "state"]
        .iter()
        .find_map(|key| address.get(*key).and_then(Value::as_str))
        .unwrap_or(display_name.as_str())
        .to_owned();
    let country_code = address
        .get("country_code")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_uppercase();
    let timezone = value
        .get("timezone")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|tz| !tz.is_empty())
        .or_else(|| country_timezone(&country_code).map(str::to_owned))
        .unwrap_or_else(|| fallback_utc_offset(longitude_of(&value)));
    Ok(ReverseGeoResult {
        city,
        country_code,
        timezone,
    })
}

/// IANA timezone for a country code. Nominatim's reverse responses on the
/// public instance do not carry a `timezone` field, so map the common ones
/// explicitly; unknown codes fall back to a longitude-derived UTC offset.
fn country_timezone(country: &str) -> Option<&'static str> {
    Some(match country {
        "GB" => "Europe/London",
        "DE" => "Europe/Berlin",
        "FR" => "Europe/Paris",
        "IT" => "Europe/Rome",
        "ES" => "Europe/Madrid",
        "NL" => "Europe/Amsterdam",
        "BE" => "Europe/Brussels",
        "CH" => "Europe/Zurich",
        "AT" => "Europe/Vienna",
        "SE" => "Europe/Stockholm",
        "NO" => "Europe/Oslo",
        "DK" => "Europe/Copenhagen",
        "FI" => "Europe/Helsinki",
        "IE" => "Europe/Dublin",
        "PT" => "Europe/Lisbon",
        "PL" => "Europe/Warsaw",
        "CZ" => "Europe/Prague",
        "GR" => "Europe/Athens",
        "RO" => "Europe/Bucharest",
        "HU" => "Europe/Budapest",
        "TR" => "Europe/Istanbul",
        "UA" => "Europe/Kyiv",
        "RU" => "Europe/Moscow",
        "US" => "America/New_York",
        "CA" => "America/Toronto",
        "MX" => "America/Mexico_City",
        "BR" => "America/Sao_Paulo",
        "AR" => "America/Argentina/Buenos_Aires",
        "CN" => "Asia/Shanghai",
        "HK" => "Asia/Hong_Kong",
        "TW" => "Asia/Taipei",
        "JP" => "Asia/Tokyo",
        "KR" => "Asia/Seoul",
        "SG" => "Asia/Singapore",
        "MY" => "Asia/Kuala_Lumpur",
        "TH" => "Asia/Bangkok",
        "VN" => "Asia/Ho_Chi_Minh",
        "PH" => "Asia/Manila",
        "ID" => "Asia/Jakarta",
        "IN" => "Asia/Kolkata",
        "PK" => "Asia/Karachi",
        "BD" => "Asia/Dhaka",
        "AE" => "Asia/Dubai",
        "SA" => "Asia/Riyadh",
        "IL" => "Asia/Jerusalem",
        "AU" => "Australia/Sydney",
        "NZ" => "Pacific/Auckland",
        "ZA" => "Africa/Johannesburg",
        "EG" => "Africa/Cairo",
        _ => return None,
    })
}

/// Approximate timezone label from the longitude (UTC +- lon/15 rounded).
fn fallback_utc_offset(longitude: f64) -> String {
    let hours = (longitude / 15.0).round() as i32;
    if hours == 0 {
        "UTC".to_owned()
    } else if hours > 0 {
        format!("UTC+{hours}")
    } else {
        format!("UTC{hours}")
    }
}

fn longitude_of(value: &Value) -> f64 {
    value
        .get("lon")
        .and_then(Value::as_str)
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn parse_lat_lon(first: &Value) -> Result<(f64, f64), GeocodeError> {
    let lat = first
        .get("lat")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<f64>().ok())
        .ok_or(GeocodeError::InvalidData)?;
    let lon = first
        .get("lon")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<f64>().ok())
        .ok_or(GeocodeError::InvalidData)?;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(GeocodeError::InvalidData);
    }
    Ok((lat, lon))
}

/// Geocode `query` against `host:port`. Port 443 uses TLS; port 80 uses a
/// plain HTTP/1.1 GET (used by local mock tests).
pub fn geocode_at(host: &str, port: u16, query: &str) -> Result<(f64, f64), GeocodeError> {
    let path = format!(
        "/search?q={}&format=json&limit=1&accept-language=en",
        urlencode(query)
    );
    let body = if port == 443 {
        https_get(host, &path)?
    } else {
        http_get(host, port, &path)?
    };
    parse_geocode_response(&body)
}

fn http_get(host: &str, port: u16, path: &str) -> Result<Vec<u8>, GeocodeError> {
    let mut stream = TcpStream::connect((host, port)).map_err(|_| GeocodeError::Unreachable)?;
    stream
        .set_read_timeout(Some(REQUEST_TIMEOUT))
        .map_err(|_| GeocodeError::Unreachable)?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUser-Agent: wloc-service/0.1\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|_| GeocodeError::Unreachable)?;
    read_body(stream)
}

fn https_get(host: &str, path: &str) -> Result<Vec<u8>, GeocodeError> {
    use rustls::pki_types::ServerName;
    use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
        .map_err(|_| GeocodeError::Unreachable)?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server_name =
        ServerName::try_from(host.to_owned()).map_err(|_| GeocodeError::Unreachable)?;
    let connection = ClientConnection::new(Arc::new(config), server_name)
        .map_err(|_| GeocodeError::Unreachable)?;

    let sock = TcpStream::connect((host, 443)).map_err(|_| GeocodeError::Unreachable)?;
    sock.set_read_timeout(Some(REQUEST_TIMEOUT))
        .map_err(|_| GeocodeError::Unreachable)?;
    let mut tls = StreamOwned::new(connection, sock);

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: wloc-service/0.1\r\nConnection: close\r\n\r\n"
    );
    tls.write_all(request.as_bytes())
        .map_err(|_| GeocodeError::Unreachable)?;
    let mut raw = Vec::new();
    tls.read_to_end(&mut raw)
        .map_err(|_| GeocodeError::Unreachable)?;
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(GeocodeError::InvalidData)?;
    Ok(raw[header_end + 4..].to_vec())
}

fn read_body(mut stream: TcpStream) -> Result<Vec<u8>, GeocodeError> {
    let mut raw = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                raw.extend_from_slice(&buffer[..count]);
                if raw.len() > MAX_RESPONSE_BYTES {
                    return Err(GeocodeError::InvalidData);
                }
            }
            Err(_) => return Err(GeocodeError::Unreachable),
        }
    }
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(GeocodeError::InvalidData)?;
    Ok(raw[header_end + 4..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_nominatim_response() {
        let body = br#"[{"place_id":1,"lat":"51.5074","lon":"-0.1278","display_name":"London"}]"#;
        let (lat, lon) = parse_geocode_response(body).unwrap();
        assert_eq!((lat, lon), (51.5074, -0.1278));
    }

    #[test]
    fn fallback_utc_offset_covers_all_three_sign_branches() {
        // The longitude-derived timezone fallback is used when reverse
        // geocoding returns no known country mapping: zero rounds to plain
        // UTC, positive longitudes become UTC+N, negative become UTC-N.
        assert_eq!(fallback_utc_offset(0.0), "UTC");
        assert_eq!(fallback_utc_offset(75.0), "UTC+5");
        assert_eq!(fallback_utc_offset(-30.0), "UTC-2");
    }

    #[test]
    fn geocode_error_display_messages() {
        assert_eq!(GeocodeError::NotFound.to_string(), "place not found");
        assert_eq!(
            GeocodeError::InvalidData.to_string(),
            "geocoder returned invalid data"
        );
        assert_eq!(
            GeocodeError::Unreachable.to_string(),
            "geocoder unreachable"
        );
    }

    #[test]
    fn parse_lat_lon_rejects_missing_or_malformed_longitude() {
        // A lon field that is absent or not parseable must fail as invalid
        // data rather than silently defaulting to longitude 0.
        assert_eq!(
            parse_lat_lon(&serde_json::from_str(r#"{"lat":"10.0"}"#).unwrap()),
            Err(GeocodeError::InvalidData)
        );
        assert_eq!(
            parse_lat_lon(&serde_json::from_str(r#"{"lat":"10.0","lon":"not-a-number"}"#).unwrap()),
            Err(GeocodeError::InvalidData)
        );
        assert_eq!(
            parse_lat_lon(&serde_json::from_str(r#"{"lat":"95","lon":"10"}"#).unwrap()),
            Err(GeocodeError::InvalidData)
        );
    }

    #[test]
    fn parse_response_with_name_extracts_display_name() {
        let body = br#"[{"lat":"35.6768601","lon":"139.7638947","display_name":"Tokyo, Japan"}]"#;
        let result = parse_geocode_response_with_name(body).unwrap();
        assert_eq!(result.city, "Tokyo, Japan");
        assert_eq!(result.latitude, 35.6768601);
        assert_eq!(result.longitude, 139.7638947);
    }

    #[test]
    fn parse_reverse_response_extracts_place_info() {
        let body = br#"{"display_name":"Berlin, Germany","address":{"city":"Berlin","country_code":"de"},"timezone":"Europe/Berlin"}"#;
        let result = parse_reverse_geocode_response(body).unwrap();
        assert_eq!(result.city, "Berlin");
        assert_eq!(result.country_code, "DE");
        assert_eq!(result.timezone, "Europe/Berlin");
    }

    #[test]
    fn reverse_without_timezone_uses_country_map() {
        let body =
            br#"{"display_name":"Tokyo, Japan","lon":"139.76","address":{"city":"Tokyo","country_code":"jp"}}"#;
        let result = parse_reverse_geocode_response(body).unwrap();
        assert_eq!(result.country_code, "JP");
        assert_eq!(result.timezone, "Asia/Tokyo");
    }

    #[test]
    fn reverse_unknown_country_uses_longitude_offset() {
        let body = br#"{"display_name":"Somewhere","lon":"-75.0","address":{"country_code":"xx"}}"#;
        let result = parse_reverse_geocode_response(body).unwrap();
        assert_eq!(result.timezone, "UTC-5");
    }

    #[test]
    fn reverse_missing_display_name_is_not_found() {
        assert_eq!(
            parse_reverse_geocode_response(b"{\"address\":{}}"),
            Err(GeocodeError::NotFound)
        );
    }

    #[test]
    fn empty_response_is_not_found() {
        assert_eq!(parse_geocode_response(b"[]"), Err(GeocodeError::NotFound));
    }

    #[test]
    fn malformed_or_out_of_range_is_invalid() {
        assert_eq!(
            parse_geocode_response(b"not-json"),
            Err(GeocodeError::InvalidData)
        );
        assert_eq!(
            parse_geocode_response(br#"[{"lat":"95","lon":"0"}]"#),
            Err(GeocodeError::InvalidData)
        );
    }

    #[test]
    fn geocode_at_parses_a_mock_http_response() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 1024];
            let _ = sock.read(&mut buf).unwrap();
            let body = br#"[{"place_id":1,"lat":"51.5074","lon":"-0.1278"}]"#;
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            sock.write_all(head.as_bytes()).unwrap();
            sock.write_all(body).unwrap();
        });
        let (lat, lon) = geocode_at("127.0.0.1", port, "London").unwrap();
        assert_eq!((lat, lon), (51.5074, -0.1278));
    }

    #[test]
    fn reverse_geocode_at_parses_a_mock_http_response_off_the_control_path() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 1024];
            let count = sock.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..count]);
            assert!(request.starts_with("GET /reverse?lat=51.5074&lon=-0.1278"));
            let body = br#"{"display_name":"London, UK","address":{"city":"London","country_code":"gb"},"timezone":"Europe/London"}"#;
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            sock.write_all(head.as_bytes()).unwrap();
            sock.write_all(body).unwrap();
        });

        let result = reverse_geocode_at("127.0.0.1", port, 51.5074, -0.1278).unwrap();
        assert_eq!(result.city, "London");
        assert_eq!(result.country_code, "GB");
        assert_eq!(result.timezone, "Europe/London");
    }

    #[test]
    fn reverse_geocode_at_rejects_malformed_and_oversized_http_responses() {
        use std::io::{Read, Write};

        fn serve_once(response: Vec<u8>) -> u16 {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            std::thread::spawn(move || {
                let (mut sock, _) = listener.accept().unwrap();
                let mut request = [0_u8; 1024];
                let _ = sock.read(&mut request).unwrap();
                sock.write_all(&response).unwrap();
            });
            port
        }

        let malformed_port = serve_once(b"HTTP/1.1 200 OK\r\nmissing separator".to_vec());
        assert_eq!(
            reverse_geocode_at("127.0.0.1", malformed_port, 1.0, 2.0),
            Err(GeocodeError::InvalidData)
        );

        let body = vec![b'x'; MAX_RESPONSE_BYTES + 1];
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let mut oversized = head.into_bytes();
        oversized.extend_from_slice(&body);
        let oversized_port = serve_once(oversized);
        assert_eq!(
            reverse_geocode_at("127.0.0.1", oversized_port, 1.0, 2.0),
            Err(GeocodeError::InvalidData)
        );
    }

    #[test]
    fn geocode_at_mock_empty_is_not_found() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 1024];
            let _ = sock.read(&mut buf).unwrap();
            let body = b"[]";
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            sock.write_all(head.as_bytes()).unwrap();
            sock.write_all(body).unwrap();
        });
        assert_eq!(
            geocode_at("127.0.0.1", port, "Nowhere"),
            Err(GeocodeError::NotFound)
        );
    }

    #[test]
    fn urlencoding_escapes_spaces_and_reserved() {
        assert_eq!(urlencode("London, UK"), "London%2C+UK");
        assert_eq!(urlencode("New York"), "New+York");
        assert_eq!(urlencode("café"), "caf%C3%A9");
    }
}
