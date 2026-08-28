//! HTTP/2 MITM proxy core.
//!
//! Terminates TLS on the client side (approved host only via the certificate
//! resolver), bridges HTTP/2 to the real upstream (verifying its certificate),
//! and intercepts the `/clls/wloc` response path to apply the WLOC patch.
//!
//! Fail-open: any interception error forwards the original upstream response
//! unchanged; a malformed WLOC response is never replaced.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http::{HeaderValue, Request, Response};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::mitm::{CaBundle, MitmCertResolver, MitmError};
use crate::wloc::{patch_wloc_response, PatchTarget};

/// The Apple WLOC endpoint path intercepted for patching.
pub const WLOC_PATH: &str = "/clls/wloc";
/// Upper bound for a single forwarded response body.
const MAX_FORWARD_BODY_BYTES: usize = 512 * 1024;
/// Concurrent upstream streams per client connection.
const MAX_STREAMS: usize = 8;
/// A client that opens an HTTP/2 stream but never finishes its POST must not
/// occupy one of the router's eight proxy slots for the full connection life.
const REQUEST_BODY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// HTTP/2 MITM proxy bound to one approved hostname's traffic.
#[derive(Clone)]
pub struct MitmProxy {
    tls_config: Arc<rustls::ServerConfig>,
    upstream_connector: TlsConnector,
    /// Test hook: override the TCP connect target while keeping the approved
    /// hostname for SNI and the Host header. Production uses hostname:443.
    upstream_override: Option<(String, u16)>,
    /// Per-host public DNS answers used only when local DNS maps the approved
    /// name back to this router for TPROXY ingress.
    upstream_override_file: Option<std::path::PathBuf>,
    /// Append-only rewrite log (one JSON line per patched response).
    events_file: Option<std::path::PathBuf>,
}

impl MitmProxy {
    /// Build the proxy with a certificate resolver over `ca` and an upstream
    /// client that verifies against `upstream_roots` (real roots in
    /// production; a test root store in tests).
    pub fn new(ca: &CaBundle, upstream_roots: rustls::RootCertStore) -> Result<Self, MitmError> {
        let resolver = MitmCertResolver::new(ca)?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let versions = [&rustls::version::TLS13, &rustls::version::TLS12];

        let mut server = rustls::ServerConfig::builder_with_provider(Arc::clone(&provider))
            .with_protocol_versions(&versions)
            .map_err(|error| MitmError::Certificate(error.to_string()))?
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(resolver));
        server.alpn_protocols = vec![b"h2".to_vec()];

        let mut client = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&versions)
            .map_err(|error| MitmError::Certificate(error.to_string()))?
            .with_root_certificates(upstream_roots)
            .with_no_client_auth();
        // The Apple WLOC endpoint serves HTTP/1.1, so the upstream client
        // offers it (and h2 as a fallback) via ALPN.
        client.alpn_protocols = vec![b"http/1.1".to_vec(), b"h2".to_vec()];

        Ok(Self {
            tls_config: Arc::new(server),
            upstream_connector: TlsConnector::from(Arc::new(client)),
            upstream_override: None,
            upstream_override_file: None,
            events_file: None,
        })
    }

    /// Record patched WLOC responses to `events_file` (one JSON line each).
    pub fn with_events_file(mut self, events_file: std::path::PathBuf) -> Self {
        self.events_file = Some(events_file);
        self
    }

    /// Point the upstream TCP connection at `host:port` instead of the
    /// approved hostname on 443. Test-only; SNI and Host stay approved.
    pub fn with_upstream_override(mut self, host: impl Into<String>, port: u16) -> Self {
        self.upstream_override = Some((host.into(), port));
        self
    }

    /// Read host-specific public upstream answers before each request.
    pub fn with_upstream_override_file(mut self, path: std::path::PathBuf) -> Self {
        self.upstream_override_file = Some(path);
        self
    }

    /// Serve one accepted client TCP connection: TLS terminate, run the H2
    /// server, and proxy each request to the upstream, patching WLOC
    /// responses. Returns once the client connection closes.
    pub async fn handle_connection(
        &self,
        client_tcp: TcpStream,
        patch_state: Arc<Mutex<Option<PatchTarget>>>,
    ) -> Result<(), MitmProxyError> {
        // TPROXY preserves the client's original Apple destination as the
        // accepted socket's local address. Reuse it so a stale first DNS
        // result can never pin every request to a dead CDN address.
        let original_destination = client_tcp
            .local_addr()
            .ok()
            .filter(is_usable_original_destination);
        let client_tls = TlsAcceptor::from(Arc::clone(&self.tls_config))
            .accept(client_tcp)
            .await
            .map_err(|error| MitmProxyError::ClientTls(error.to_string()))?;

        let mut server = h2::server::Builder::new()
            .initial_window_size(32 * 1024)
            .max_frame_size(16 * 1024)
            .max_concurrent_streams(MAX_STREAMS as u32)
            .handshake::<_, Bytes>(client_tls)
            .await
            .map_err(|error| MitmProxyError::H2(error.to_string()))?;

        while let Some(accepted) = server.accept().await {
            let request = match accepted {
                Ok(request) => request,
                Err(_) => break,
            };
            let (request, mut respond) = request;
            let proxy = self.clone();
            let patch_state = Arc::clone(&patch_state);
            tokio::spawn(async move {
                // iOS opens speculative POST streams beside the stream that
                // carries the actual Wi-Fi scan. Keep one idle stream from
                // delaying the other (the H2 server caps this at MAX_STREAMS).
                let patch = patch_state.lock().ok().and_then(|guard| *guard);
                match proxy
                    .forward_upstream(request, patch.as_ref(), original_destination)
                    .await
                {
                    Ok((status, headers, original_len, patched_body, rewritten)) => {
                        proxy.append_rewrite_event(
                            patch.as_ref(),
                            original_len,
                            patched_body.len(),
                            rewritten,
                        );
                        let mut response = Response::new(());
                        *response.status_mut() = status;
                        for (name, value) in headers {
                            if let Some(name) = name {
                                if !matches!(
                                    name.as_str(),
                                    "connection"
                                        | "content-length"
                                        | "keep-alive"
                                        | "proxy-connection"
                                        | "transfer-encoding"
                                        | "upgrade"
                                ) {
                                    response.headers_mut().append(name, value);
                                }
                            }
                        }
                        if let Ok(mut send) =
                            respond.send_response(response, patched_body.is_empty())
                        {
                            if !patched_body.is_empty() {
                                let _ = send.send_data(Bytes::from(patched_body), true);
                            }
                        }
                    }
                    Err(error) => eprintln!("wloc proxy: upstream failure: {error}"),
                }
            });
        }
        Ok(())
    }

    /// Forward one client request to the real upstream over a fresh verified
    /// TLS + H2 connection, patching a `/clls/wloc` response body. Returns
    /// only the (possibly patched) response body; headers are discarded so a
    /// stale `content-length` never misleads the client.
    async fn forward_upstream(
        &self,
        request: Request<h2::RecvStream>,
        patch: Option<&PatchTarget>,
        original_destination: Option<SocketAddr>,
    ) -> Result<(http::StatusCode, http::HeaderMap, usize, Vec<u8>, bool), MitmProxyError> {
        let hostname = approved_host(&request)?;
        let is_wloc =
            request.uri().path() == WLOC_PATH || request.uri().path().ends_with("/clls/wloc");
        eprintln!(
            "wloc proxy: request host={hostname} method={} is_wloc={is_wloc}",
            request.method()
        );

        // Read the bounded client request body.
        let mut request_body = Vec::new();
        let (parts, mut client_body) = request.into_parts();
        while let Some(chunk) = tokio::time::timeout(REQUEST_BODY_TIMEOUT, client_body.data())
            .await
            .map_err(|_| MitmProxyError::Upstream("request body timeout".into()))?
        {
            let chunk = chunk.map_err(|error| MitmProxyError::H2(error.to_string()))?;
            request_body.extend_from_slice(&chunk);
            if request_body.len() > MAX_FORWARD_BODY_BYTES {
                return Err(MitmProxyError::Upstream(
                    "request body exceeds bound".into(),
                ));
            }
        }
        eprintln!("wloc proxy: request body {} bytes", request_body.len());

        let dynamic_overrides =
            upstream_override_for(self.upstream_override_file.as_deref(), &hostname);
        let connect_targets = choose_upstream_targets(
            original_destination,
            self.upstream_override.as_ref(),
            &dynamic_overrides,
            &hostname,
        );
        let server_name = rustls::pki_types::ServerName::try_from(hostname.clone())
            .map_err(|_| MitmProxyError::Upstream("invalid upstream hostname".into()))?;
        let mut last_error = None;
        let mut upstream_tls = None;
        for (connect_host, connect_port) in connect_targets {
            let result = async {
                let connect = TcpStream::connect((connect_host.as_str(), connect_port)).await?;
                self.upstream_connector
                    .connect(server_name.clone(), connect)
                    .await
            }
            .await;
            match result {
                Ok(stream) => {
                    upstream_tls = Some(stream);
                    break;
                }
                Err(error) => last_error = Some(error.to_string()),
            }
        }
        let upstream_tls = upstream_tls.ok_or_else(|| {
            MitmProxyError::Upstream(last_error.unwrap_or_else(|| "no upstream target".into()))
        })?;

        // The real Apple /clls/wloc endpoint serves HTTP/1.1; an h2 upstream
        // fails with "frame with invalid size". Forward over HTTP/1.1 and
        // decode Content-Length / chunked bodies.
        let upstream_request = sanitized_forward_request(parts, &hostname)?;
        let (status, headers, body) =
            crate::mitm::http1::forward_http1(upstream_tls, &upstream_request, &request_body)
                .await?;

        let patched = maybe_patch_body(&body, is_wloc, patch);
        let rewritten = patched != body;
        eprintln!(
            "wloc proxy: response body {} -> {} bytes (is_wloc={is_wloc}, patch={})",
            body.len(),
            patched.len(),
            patch.is_some()
        );
        Ok((status, headers, body.len(), patched, rewritten))
    }

    /// Append one rewrite event per patched WLOC response.
    fn append_rewrite_event(
        &self,
        patch: Option<&PatchTarget>,
        before: usize,
        after: usize,
        rewritten: bool,
    ) {
        let Some(events_file) = &self.events_file else {
            return;
        };
        if !rewritten {
            return;
        }
        let event = serde_json::json!({
            "type": "rewritten",
            "time": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            "latitude": patch.map(|t| t.latitude),
            "longitude": patch.map(|t| t.longitude),
            "bytes_before": before,
            "bytes_after": after,
        });
        crate::service::append_event_line(events_file, &event);
    }
}

fn is_usable_original_destination(address: &SocketAddr) -> bool {
    address.port() == 443
        && match address.ip() {
            std::net::IpAddr::V4(ip) => {
                !ip.is_unspecified() && !ip.is_loopback() && !ip.is_private() && !ip.is_link_local()
            }
            std::net::IpAddr::V6(ip) => !ip.is_unspecified() && !ip.is_loopback(),
        }
}

/// A stable local DNS ingress has no usable Apple destination to reuse, so
/// resolve the requested hostname instead of reusing another host's CDN IP.
fn choose_upstream_targets(
    original_destination: Option<SocketAddr>,
    explicit_override: Option<&(String, u16)>,
    dynamic_overrides: &[(String, u16)],
    hostname: &str,
) -> Vec<(String, u16)> {
    if let Some((host, port)) = explicit_override {
        return vec![(host.clone(), *port)];
    }
    if let Some(address) = original_destination.filter(is_usable_original_destination) {
        return vec![(address.ip().to_string(), address.port())];
    }
    if !dynamic_overrides.is_empty() {
        return dynamic_overrides.to_vec();
    }
    vec![(hostname.to_owned(), 443)]
}

fn upstream_override_for(path: Option<&std::path::Path>, hostname: &str) -> Vec<(String, u16)> {
    let Some(path) = path else {
        return Vec::new();
    };
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    // DNS may return several CDN addresses; retry only this hostname's first
    // four answers, always with strict TLS verification.
    contents
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let configured_host = fields.next()?;
            let address = fields.next()?.parse::<std::net::IpAddr>().ok()?;
            configured_host
                .eq_ignore_ascii_case(hostname)
                .then_some((address.to_string(), 443))
        })
        .take(4)
        .collect()
}

/// Reject requests whose authority is not one of the approved hosts.
/// Returns an owned hostname so the caller can move the request afterwards.
fn approved_host<B>(request: &Request<B>) -> Result<String, MitmProxyError> {
    let authority = request
        .uri()
        .authority()
        .map(|value| value.as_str())
        .ok_or(MitmProxyError::Upstream("missing authority".into()))?;
    let hostname = authority.trim_start_matches("https://");
    let hostname = hostname
        .split(':')
        .next()
        .ok_or(MitmProxyError::Upstream("invalid authority".into()))?;
    let hostname = hostname.trim_end_matches('.');
    crate::APPROVED_WLOC_HOSTS
        .iter()
        .find(|approved| approved.eq_ignore_ascii_case(hostname))
        .map(|approved| (*approved).to_owned())
        .ok_or_else(|| MitmProxyError::Upstream(format!("host not approved: {hostname}")))
}

/// Strip hop-by-hop and authority headers from the forwarded request so the
/// upstream sees a clean, bounded request.
fn sanitized_forward_request(
    parts: http::request::Parts,
    hostname: &str,
) -> Result<Request<()>, MitmProxyError> {
    let mut parts = parts;
    for header in [
        "host",
        "connection",
        "proxy-connection",
        "keep-alive",
        "transfer-encoding",
        "upgrade",
        "te",
    ] {
        parts.headers.remove(header);
    }
    if let Ok(value) = HeaderValue::from_str(hostname) {
        parts.headers.insert("host", value);
    }
    Ok(Request::from_parts(parts, ()))
}

fn maybe_patch_body(body: &[u8], is_wloc: bool, patch: Option<&PatchTarget>) -> Vec<u8> {
    match patch {
        Some(patch) if is_wloc => patch_wloc_response(body, patch),
        _ => body.to_vec(),
    }
}

/// Proxy-level failure; the caller treats any error as "close this
/// connection" and never fabricates a response.
#[derive(Clone, Debug)]
pub enum MitmProxyError {
    /// The client-side TLS termination failed (including a non-approved SNI).
    ClientTls(String),
    /// An HTTP/2 protocol error on either side.
    H2(String),
    /// The upstream connection, forwarding, or response failed.
    Upstream(String),
}

impl std::fmt::Display for MitmProxyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientTls(message) => write!(formatter, "mitm client TLS: {message}"),
            Self::H2(message) => write!(formatter, "mitm H2: {message}"),
            Self::Upstream(message) => write!(formatter, "mitm upstream: {message}"),
        }
    }
}

impl std::error::Error for MitmProxyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_original_destination_wins_over_hostname_fallback() {
        let original = Some("203.0.113.10:443".parse().unwrap());
        let dynamic = vec![("203.0.113.20".to_owned(), 443)];
        assert_eq!(
            choose_upstream_targets(original, None, &dynamic, "gs-loc.apple.com"),
            vec![("203.0.113.10".to_owned(), 443)]
        );
    }

    #[test]
    fn local_dns_ingress_uses_the_requested_hostname() {
        let original = Some("192.168.31.1:443".parse().unwrap());
        let dynamic = vec![
            ("140.205.31.96".to_owned(), 443),
            ("140.205.31.97".to_owned(), 443),
        ];
        assert_eq!(
            choose_upstream_targets(original, None, &dynamic, "gs-loc-cn.apple.com"),
            dynamic
        );
    }

    #[test]
    fn host_specific_override_never_reuses_another_hosts_address() {
        let path = std::env::temp_dir().join(format!(
            "wloc-upstream-map-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            "gs-loc.apple.com 17.252.196.22\ngs-loc-cn.apple.com 140.205.31.96\ngs-loc-cn.apple.com 140.205.31.97\n",
        )
        .unwrap();

        assert_eq!(
            upstream_override_for(Some(path.as_path()), "gs-loc-cn.apple.com"),
            vec![
                ("140.205.31.96".to_owned(), 443),
                ("140.205.31.97".to_owned(), 443),
            ]
        );
        assert_eq!(
            upstream_override_for(Some(path.as_path()), "gsp-ssl.ls.apple.com"),
            Vec::<(String, u16)>::new()
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn approved_host_normalizes_dns_case_and_root_dot() {
        let request = Request::builder()
            .uri("https://GS-LOC.APPLE.COM.:443/clls/wloc")
            .body(())
            .unwrap();
        assert_eq!(approved_host(&request).unwrap(), "gs-loc.apple.com");
    }
}
