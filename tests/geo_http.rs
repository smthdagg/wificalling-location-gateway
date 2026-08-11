//! Real HTTP Geo provider adapter contract.
//!
//! A bounded blocking HTTP/1.1 GET against a Geo provider (default
//! ip-api.com) drives the GeoProviderRuntime boundary. Response parsing is a
//! pure function tested offline; a local mock server exercises the full HTTP
//! round trip without the network.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::thread;
use std::time::Duration;

use wificalling_location_gateway::georesolver::http::{
    parse_geo_response, GeoHttpClient, MAX_RESPONSE_BYTES,
};
use wificalling_location_gateway::georesolver::runtime::{GeoProviderRuntime, ProviderFailure};
use wificalling_location_gateway::georesolver::ProviderRef;

const EXIT_V4: IpAddr = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
const NOW: u64 = 1_000_000;

const CANNED_BODY: &str = r#"{"status":"success","countryCode":"US","lat":39.03,"lon":-77.5,"timezone":"America/New_York"}"#;

fn http_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

// --- pure parse function ---

#[test]
fn success_response_parses_into_a_valid_record() {
    let (resolved_ip, record) = parse_geo_response(EXIT_V4, NOW, CANNED_BODY.as_bytes())
        .unwrap()
        .expect("success response must yield a record");

    assert_eq!(resolved_ip, EXIT_V4);
    assert_eq!(record.country_code, "US");
    assert_eq!(record.latitude, 39.03);
    assert_eq!(record.longitude, -77.5);
    assert_eq!(record.timezone, "America/New_York");
    assert_eq!(record.expires_at_unix, NOW + 3_600);
}

#[test]
fn fail_status_is_no_data_not_an_error() {
    let body = br#"{"status":"fail","message":"invalid query"}"#;
    assert_eq!(parse_geo_response(EXIT_V4, NOW, body).unwrap(), None);
}

#[test]
fn malformed_or_missing_fields_are_invalid_data() {
    for body in [
        b"not-json".as_slice(),
        br#"{"status":"success"}"#.as_slice(),
        br#"{"status":"success","countryCode":"US","lat":"bad","lon":-77.5,"timezone":"America/New_York"}"#.as_slice(),
        br#"{"status":"success","countryCode":"US","lat":39.03,"lon":-77.5}"#.as_slice(),
        br#"{"status":"success","countryCode":"ZZ","lat":39.03,"lon":-77.5,"timezone":"America/New_York"}"#.as_slice(),
    ] {
        assert_eq!(
            parse_geo_response(EXIT_V4, NOW, body),
            Err(ProviderFailure::InvalidData),
            "unexpected result for {body:?}"
        );
    }
}

// --- HTTP round trip against a local mock server (offline) ---

fn spawn_mock_server(response: String) -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("mock server accept");
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = [0_u8; 2048];
        let _ = socket.read(&mut request).unwrap();
        socket.write_all(response.as_bytes()).unwrap();
    });
    port
}

#[test]
fn client_round_trips_a_success_response_offline() {
    let port = spawn_mock_server(http_response(CANNED_BODY));
    let mut client = GeoHttpClient::new("127.0.0.1", port);

    let (resolved_ip, record) = client
        .lookup(ProviderRef::new("local").unwrap(), EXIT_V4)
        .unwrap()
        .expect("mock success must yield a record");
    assert_eq!(resolved_ip, EXIT_V4);
    assert_eq!(record.country_code, "US");
}

#[test]
fn client_handles_non_200_and_fail_status_as_no_data_or_error() {
    // Non-200 status is invalid data.
    let port = spawn_mock_server(
        "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
    );
    let mut client = GeoHttpClient::new("127.0.0.1", port);
    assert_eq!(
        client.lookup(ProviderRef::new("local").unwrap(), EXIT_V4),
        Err(ProviderFailure::InvalidData)
    );

    // A fail status is no data, not an error.
    let port = spawn_mock_server(http_response(r#"{"status":"fail"}"#));
    let mut client = GeoHttpClient::new("127.0.0.1", port);
    assert_eq!(
        client
            .lookup(ProviderRef::new("local").unwrap(), EXIT_V4)
            .unwrap(),
        None
    );
}

#[test]
fn client_fails_fast_on_unreachable_host() {
    // Bind and drop a listener so the port is guaranteed dead.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut client = GeoHttpClient::new("127.0.0.1", dead_port);
    let result = client.lookup(ProviderRef::new("local").unwrap(), EXIT_V4);
    assert!(matches!(result, Err(ProviderFailure::Unreachable)));
}

#[test]
fn oversized_response_is_invalid_data() {
    let body = "x".repeat(MAX_RESPONSE_BYTES + 1);
    let port = spawn_mock_server(http_response(&body));
    let mut client = GeoHttpClient::new("127.0.0.1", port);
    assert_eq!(
        client.lookup(ProviderRef::new("local").unwrap(), EXIT_V4),
        Err(ProviderFailure::InvalidData)
    );
}

/// Live smoke test against the real ip-api.com endpoint. Requires network;
/// run explicitly with `cargo test --test geo_http -- --ignored`.
#[test]
#[ignore = "requires network access to ip-api.com"]
fn live_ip_api_lookup_returns_a_valid_record() {
    let mut client = GeoHttpClient::ip_api_default();
    let result = client
        .lookup(ProviderRef::new("ip-api").unwrap(), EXIT_V4)
        .expect("live lookup must not fail with a transport error");

    let (resolved_ip, record) = result.expect("a public exit must resolve to a record");
    assert_eq!(resolved_ip, EXIT_V4);
    assert!(
        record
            .validate_at(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            )
            .is_ok(),
        "live record must pass semantic validation"
    );
    println!(
        "live geo for {EXIT_V4}: {} ({}, {})",
        record.country_code, record.latitude, record.longitude
    );
}
