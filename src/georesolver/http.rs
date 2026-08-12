//! Bounded HTTP/1.1 Geo provider adapter.
//!
//! A small blocking HTTP GET (bounded bytes, bounded timeouts) drives the
//! [`GeoProviderRuntime`] boundary against a Geo provider such as ip-api.com.
//! Response parsing is a pure function so it is fully testable offline; the
//! transport is exercised against a local mock server in tests.
//!
//! The blocking transport is a PoC simplification: status lookups are cached
//! by the composed service, so lookups are infrequent and each is bounded by
//! [`REQUEST_TIMEOUT`]. A production adapter may switch to an async client.

use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::service::GeoRecord;

use super::runtime::{GeoProviderRuntime, ProviderFailure};
use super::ProviderRef;

/// Geo record TTL in seconds, within the service's MAX_GEO_TTL_SECONDS bound.
const GEO_TTL_SECONDS: u64 = 3_600;
/// Total transport bound for one lookup.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound for the full HTTP response (headers + body).
pub const MAX_RESPONSE_BYTES: usize = 16 * 1024;

fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Parse an ip-api.com-style Geo response into a record for `ip`.
///
/// `status != "success"` is treated as "no data" (the provider answered but
/// could not resolve the address); missing or malformed fields are invalid
/// data. The record expiry is `now_unix + GEO_TTL_SECONDS`.
pub fn parse_geo_response(
    ip: IpAddr,
    now_unix: u64,
    body: &[u8],
) -> Result<Option<(IpAddr, GeoRecord)>, ProviderFailure> {
    let value: Value = serde_json::from_slice(body).map_err(|_| ProviderFailure::InvalidData)?;
    if value.get("status").and_then(Value::as_str) != Some("success") {
        return Ok(None);
    }
    let country_code = value
        .get("countryCode")
        .and_then(Value::as_str)
        .ok_or(ProviderFailure::InvalidData)?;
    let city = value
        .get("city")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let latitude = value
        .get("lat")
        .and_then(Value::as_f64)
        .ok_or(ProviderFailure::InvalidData)?;
    let longitude = value
        .get("lon")
        .and_then(Value::as_f64)
        .ok_or(ProviderFailure::InvalidData)?;
    let timezone = value
        .get("timezone")
        .and_then(Value::as_str)
        .ok_or(ProviderFailure::InvalidData)?;

    let record = GeoRecord {
        country_code: country_code.to_owned(),
        city,
        latitude,
        longitude,
        timezone: timezone.to_owned(),
        expires_at_unix: now_unix + GEO_TTL_SECONDS,
    };
    if record.validate_at(now_unix).is_err() {
        return Err(ProviderFailure::InvalidData);
    }
    Ok(Some((ip, record)))
}

/// Perform a bounded HTTP/1.1 GET and return the response body.
fn http_get(host: &str, port: u16, path: &str) -> Result<Vec<u8>, ProviderFailure> {
    let started = SystemTime::now();
    // Resolve every address and prefer IPv4: some networks (notably behind a
    // NAT where IPv6 has no working path) resolve the provider to IPv6 first
    // and stall, burning the whole request deadline. Trying addresses in
    // order with a per-connect timeout reaches the first reachable one.
    let mut addresses: Vec<std::net::SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|_| ProviderFailure::Unreachable)?
        .collect();
    if addresses.is_empty() {
        return Err(ProviderFailure::Unreachable);
    }
    addresses.sort_by_key(|address| !address.is_ipv4());

    let mut stream = None;
    for address in addresses {
        if SystemTime::now()
            .duration_since(started)
            .map(|elapsed| elapsed > REQUEST_TIMEOUT)
            .unwrap_or(true)
        {
            return Err(ProviderFailure::Timeout);
        }
        if let Ok(candidate) = TcpStream::connect_timeout(&address, REQUEST_TIMEOUT) {
            stream = Some(candidate);
            break;
        }
    }
    let mut stream = stream.ok_or(ProviderFailure::Unreachable)?;
    stream
        .set_read_timeout(Some(REQUEST_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(REQUEST_TIMEOUT)))
        .map_err(|_| ProviderFailure::Unreachable)?;

    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\nUser-Agent: wloc-service/0.1\r\nAccept: application/json\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|_| ProviderFailure::Unreachable)?;

    let mut raw = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        if SystemTime::now()
            .duration_since(started)
            .map(|elapsed| elapsed > REQUEST_TIMEOUT)
            .unwrap_or(true)
        {
            return Err(ProviderFailure::Timeout);
        }
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                raw.extend_from_slice(&buffer[..count]);
                if raw.len() > MAX_RESPONSE_BYTES {
                    return Err(ProviderFailure::InvalidData);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(ProviderFailure::Timeout);
            }
            Err(_) => return Err(ProviderFailure::Unreachable),
        }
    }

    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(ProviderFailure::InvalidData)?;
    let header = &raw[..header_end];
    let mut body = &raw[header_end + 4..];

    let status_line = header
        .split(|byte| *byte == b'\n')
        .next()
        .ok_or(ProviderFailure::InvalidData)?;
    let code: u16 = status_line
        .split(|byte| *byte == b' ')
        .nth(1)
        .and_then(|part| std::str::from_utf8(part).ok())
        .and_then(|part| part.trim().parse().ok())
        .ok_or(ProviderFailure::InvalidData)?;
    if code != 200 {
        return Err(ProviderFailure::InvalidData);
    }

    let mut content_length = None;
    for line in header.split(|byte| *byte == b'\n') {
        let line = String::from_utf8_lossy(line);
        let line = line.trim_end_matches('\r').to_ascii_lowercase();
        if let Some(value) = line.strip_prefix("content-length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }
    if let Some(length) = content_length {
        if body.len() < length {
            return Err(ProviderFailure::InvalidData);
        }
        body = &body[..length];
    }
    Ok(body.to_vec())
}

/// Blocking Geo provider adapter querying a bounded HTTP endpoint.
pub struct GeoHttpClient {
    host: String,
    port: u16,
}

impl GeoHttpClient {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    /// The default provider used by the daemon (ip-api.com, plain HTTP).
    pub fn ip_api_default() -> Self {
        Self::new("ip-api.com", 80)
    }
}

impl GeoProviderRuntime for GeoHttpClient {
    fn lookup(
        &mut self,
        _provider: ProviderRef,
        ip: IpAddr,
    ) -> Result<Option<(IpAddr, GeoRecord)>, ProviderFailure> {
        let path = format!("/json/{ip}?fields=status,countryCode,regionName,city,lat,lon,timezone");
        // The TTL starts at the query's start time, not when the response
        // arrives: a slow connect/read must not push `expires_at` past the
        // consumer's MAX_GEO_TTL_SECONDS check.
        let queried_at_unix = current_unix();
        let body = http_get(&self.host, self.port, &path)
            .inspect_err(|error| eprintln!("wloc geo lookup {ip}: transport {error:?}"))?;
        let result = parse_geo_response(ip, queried_at_unix, &body);
        match &result {
            Ok(Some((_, record))) => eprintln!(
                "wloc geo lookup {ip}: ok country={} expires={}",
                record.country_code, record.expires_at_unix
            ),
            Ok(None) => eprintln!("wloc geo lookup {ip}: no data"),
            Err(error) => eprintln!("wloc geo lookup {ip}: parse {error:?}"),
        }
        result
    }
}
