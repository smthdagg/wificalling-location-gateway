//! End-to-end MITM proxy contract: a client that trusts the MITM root CA
//! sends a `/clls/wloc` request through the proxy; the proxy terminates TLS,
//! bridges HTTP/2 to a mock upstream, patches the WLOC response with the
//! target coordinates, and the client receives the patched body.

use std::sync::Arc;

use bytes::Bytes;
use http::{Request, Response};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, TlsConnector};

use wificalling_location_gateway::mitm::proxy::MitmProxy;
use wificalling_location_gateway::mitm::{CaBundle, MitmCertResolver};
use wificalling_location_gateway::wloc::{
    coord_to_int, encode_length_delimited_field, encode_varint_field, PatchTarget,
};

const SF_LAT: i64 = 3_777_490_000;
const SF_LON: i64 = -12_241_940_000;
const LONDON_LAT: f64 = 51.5074;
const LONDON_LON: f64 = -0.1278;

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn server_config(ca: &CaBundle) -> rustls::ServerConfig {
    let resolver = MitmCertResolver::new(ca).expect("resolver builds");
    let mut config = rustls::ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS 1.3")
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(resolver));
    config.alpn_protocols = vec![b"h2".to_vec()];
    config
}

fn client_config(ca: &CaBundle) -> rustls::ClientConfig {
    let roots = ca.root_store().expect("root store builds");
    let mut config = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS 1.3")
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    config
}

/// Build a synthetic `/clls/wloc` response body: 8-byte prefix + u16 BE
/// length + payload containing one WifiDevice with a Location.
fn synthetic_wloc_body(lat: i64, lon: i64) -> Vec<u8> {
    let location = [
        encode_varint_field(1, lat),
        encode_varint_field(2, lon),
        encode_length_delimited_field(7, b"unknown-preserved"),
    ]
    .concat();
    let wifi = encode_length_delimited_field(2, &location);
    let payload = encode_length_delimited_field(2, &wifi);

    let mut body = Vec::new();
    body.extend_from_slice(&[0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00]);
    body.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    body.extend_from_slice(&payload);
    body
}

/// Extract the Location latitude (field 1) from a synthetic WLOC body.
fn extract_wifi_latitude(body: &[u8]) -> Option<i64> {
    // Envelope: prefix(8) + u16 len + payload.
    if body.len() < 10 {
        return None;
    }
    let length = u16::from_be_bytes([body[8], body[9]]) as usize;
    let payload = body.get(10..10 + length)?;
    // Root payload: find field 2 (wifi), skip key+len, parse nested.
    let (_, _, wifi_value) = find_length_delimited(payload, 2)?;
    let (_, _, location_value) = find_length_delimited(&wifi_value, 2)?;
    let (_, _, lat_value) = find_length_delimited(&location_value, 1)?;
    Some(decode_varint(&lat_value)? as i64)
}

fn decode_varint_at(bytes: &[u8], start: usize) -> Option<(u64, usize)> {
    let mut value = 0_u64;
    for (index, byte) in bytes.iter().skip(start).enumerate() {
        value |= u64::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Some((value, start + index + 1));
        }
    }
    None
}

fn decode_varint(bytes: &[u8]) -> Option<u64> {
    decode_varint_at(bytes, 0).map(|(value, _)| value)
}

fn find_length_delimited(bytes: &[u8], want: u32) -> Option<(u32, u8, Vec<u8>)> {
    let mut offset = 0;
    while offset < bytes.len() {
        let (key, key_end) = decode_varint_at(bytes, offset)?;
        let field_number = (key >> 3) as u32;
        let wire_type = (key & 0x7) as u8;
        if wire_type == 2 {
            let (length, len_end) = decode_varint_at(bytes, key_end)?;
            let end = len_end + length as usize;
            let value = bytes.get(len_end..end)?.to_vec();
            if field_number == want {
                return Some((field_number, wire_type, value));
            }
            offset = end;
        } else if wire_type == 0 {
            let (_, value_end) = decode_varint_at(bytes, key_end)?;
            if field_number == want {
                let value = bytes.get(key_end..value_end)?.to_vec();
                return Some((field_number, wire_type, value));
            }
            offset = value_end;
        } else {
            return None;
        }
    }
    None
}

#[tokio::test]
async fn wloc_response_is_patched_through_the_proxy() {
    // --- mock upstream (its own CA so the proxy can verify it) ---
    let upstream_ca = CaBundle::generate().unwrap();
    let upstream_server_config = server_config(&upstream_ca);
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_port = upstream_listener.local_addr().unwrap().port();
    let upstream_body = synthetic_wloc_body(SF_LAT, SF_LON);
    tokio::spawn(async move {
        let (stream, _) = upstream_listener.accept().await.unwrap();
        let tls = TlsAcceptor::from(Arc::new(upstream_server_config));
        let tls = tls.accept(stream).await.unwrap();
        let mut server = h2::server::Builder::new()
            .handshake::<_, Bytes>(tls)
            .await
            .unwrap();
        while let Some(accepted) = server.accept().await {
            let (request, mut respond) = accepted.unwrap();
            let _ = request.into_parts();
            let mut send = respond
                .send_response(Response::new(()), upstream_body.is_empty())
                .unwrap();
            send.send_data(Bytes::from(upstream_body.clone()), true)
                .unwrap();
        }
    });

    // --- proxy in front of the mock upstream ---
    let mitm_ca = CaBundle::generate().unwrap();
    let proxy = MitmProxy::new(&mitm_ca, upstream_ca.root_store().unwrap())
        .unwrap()
        .with_upstream_override("127.0.0.1", upstream_port);
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_port = proxy_listener.local_addr().unwrap().port();
    let target = PatchTarget::new(LONDON_LAT, LONDON_LON);
    tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) = proxy_listener.accept().await {
                let proxy = proxy.clone();
                let target = target;
                tokio::spawn(async move {
                    let _ = proxy.handle_connection(stream, &target).await;
                });
            }
        }
    });

    // --- client: trusts the MITM CA, sends a WLOC request ---
    let client_tcp = TcpStream::connect(("127.0.0.1", proxy_port)).await.unwrap();
    let connector = TlsConnector::from(Arc::new(client_config(&mitm_ca)));
    let server_name = rustls::pki_types::ServerName::try_from("gs-loc.apple.com").unwrap();
    let client_tls = connector.connect(server_name, client_tcp).await.unwrap();

    let (mut send_request, connection) = h2::client::Builder::new()
        .handshake::<_, Bytes>(client_tls)
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });

    let request = Request::builder()
        .method("GET")
        .uri("https://gs-loc.apple.com/clls/wloc")
        .body(())
        .unwrap();
    let (response_future, _send) = send_request.send_request(request, true).unwrap();
    let mut response = response_future.await.unwrap();
    let mut body = Vec::new();
    while let Some(chunk) = response.body_mut().data().await {
        body.extend_from_slice(&chunk.unwrap());
    }

    // The client must receive the PATCHED coordinates (London, not SF).
    let lat = extract_wifi_latitude(&body).expect("patched body must parse");
    assert_eq!(lat, coord_to_int(LONDON_LAT));
    assert_ne!(lat, SF_LAT, "original coordinates must be replaced");
}

#[tokio::test]
async fn non_wloc_path_passes_through_unchanged() {
    // Same setup but the upstream returns a plain body for a non-WLOC path.
    let upstream_ca = CaBundle::generate().unwrap();
    let upstream_server_config = server_config(&upstream_ca);
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_port = upstream_listener.local_addr().unwrap().port();
    let plain_body = b"plain-response".to_vec();
    let plain_body_inner = plain_body.clone();
    tokio::spawn(async move {
        let (stream, _) = upstream_listener.accept().await.unwrap();
        let tls = TlsAcceptor::from(Arc::new(upstream_server_config));
        let tls = tls.accept(stream).await.unwrap();
        let mut server = h2::server::Builder::new()
            .handshake::<_, Bytes>(tls)
            .await
            .unwrap();
        while let Some(accepted) = server.accept().await {
            let (_request, mut respond) = accepted.unwrap();
            let mut send = respond
                .send_response(Response::new(()), plain_body_inner.is_empty())
                .unwrap();
            send.send_data(Bytes::from(plain_body_inner.clone()), true)
                .unwrap();
        }
    });

    let mitm_ca = CaBundle::generate().unwrap();
    let proxy = MitmProxy::new(&mitm_ca, upstream_ca.root_store().unwrap())
        .unwrap()
        .with_upstream_override("127.0.0.1", upstream_port);
    let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_port = proxy_listener.local_addr().unwrap().port();
    let target = PatchTarget::new(LONDON_LAT, LONDON_LON);
    tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) = proxy_listener.accept().await {
                let proxy = proxy.clone();
                let target = target;
                tokio::spawn(async move {
                    let _ = proxy.handle_connection(stream, &target).await;
                });
            }
        }
    });

    let client_tcp = TcpStream::connect(("127.0.0.1", proxy_port)).await.unwrap();
    let connector = TlsConnector::from(Arc::new(client_config(&mitm_ca)));
    let server_name = rustls::pki_types::ServerName::try_from("gs-loc.apple.com").unwrap();
    let client_tls = connector.connect(server_name, client_tcp).await.unwrap();
    let (mut send_request, connection) = h2::client::Builder::new()
        .handshake::<_, Bytes>(client_tls)
        .await
        .unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });

    let request = Request::builder()
        .method("GET")
        .uri("https://gs-loc.apple.com/other/path")
        .body(())
        .unwrap();
    let (response_future, _send) = send_request.send_request(request, true).unwrap();
    let mut response = response_future.await.unwrap();
    let mut body = Vec::new();
    while let Some(chunk) = response.body_mut().data().await {
        body.extend_from_slice(&chunk.unwrap());
    }

    assert_eq!(
        body, plain_body,
        "non-WLOC responses must pass through unchanged"
    );
}
