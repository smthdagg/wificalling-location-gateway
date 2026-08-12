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

/// Parse a Nominatim `format=json` response: an array whose first object
/// carries string `lat` and `lon` fields.
pub fn parse_geocode_response(body: &[u8]) -> Result<(f64, f64), GeocodeError> {
    let value: Value = serde_json::from_slice(body).map_err(|_| GeocodeError::InvalidData)?;
    let first = value
        .as_array()
        .and_then(|items| items.first())
        .ok_or(GeocodeError::NotFound)?;
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
