//! HTTP/2 MITM proxy core.
//!
//! Terminates TLS on the client side (approved host only via the certificate
//! resolver), bridges HTTP/2 to the real upstream (verifying its certificate),
//! and intercepts the `/clls/wloc` response path to apply the WLOC patch.
//!
//! Fail-open: any interception error forwards the original upstream response
//! unchanged; a malformed WLOC response is never replaced.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use http::{HeaderValue, Request, Response};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::diagnostics::append_json_line;
use crate::mitm::{CaBundle, MitmCertResolver, MitmError};
use crate::wloc::{patch_wloc_response, PatchTarget};

/// The Apple WLOC endpoint path intercepted for patching.
pub const WLOC_PATH: &str = "/clls/wloc";

/// Resolves a patch target from the source address of an accepted client
/// connection. Implementations must return `None` for unknown, disabled, or
/// degraded profiles; the proxy then forwards the original response.
pub trait PatchTargetResolver: Send + Sync {
    fn resolve_patch_target(&self, source: IpAddr) -> Option<PatchTarget>;
}

impl PatchTargetResolver for crate::service::profile_dispatch::ProfilePatchRouter {
    fn resolve_patch_target(&self, source: IpAddr) -> Option<PatchTarget> {
        self.resolve_ip(source)
    }
}
/// Upper bound for a single forwarded response body.
const MAX_FORWARD_BODY_BYTES: usize = 512 * 1024;
/// Concurrent upstream streams per client connection.
const MAX_STREAMS: usize = 8;
/// Resource bounds for the per-client synthesis cache on small gateways.
const MAX_SYNTHESIZED_CLIENTS: usize = 16;
const MAX_SYNTHESIZED_CACHE_BYTES: usize = 64 * 1024;
const MAX_SYNTHESIZED_PAYLOAD_BYTES: usize = 16 * 1024;
const MAX_DEBUG_SAMPLE_BYTES: usize = 16 * 1024;

/// HTTP/2 MITM proxy bound to one approved hostname's traffic.
#[derive(Clone)]
pub struct MitmProxy {
    tls_config: Arc<rustls::ServerConfig>,
    upstream_connector: TlsConnector,
    /// Test hook: override the TCP connect target while keeping the approved
    /// hostname for SNI and the Host header. Production uses hostname:443.
    upstream_override: Option<(String, u16)>,
    /// Append-only rewrite log (one JSON line per patched response).
    events_file: Option<std::path::PathBuf>,
    /// Per-client cache of the last synthesized BlockBSSIDApple payload
    /// (visible APs at the target). Coordinate queries (kind 3) carry no
    /// BSSIDs of their own, so they are answered from here instead of with
    /// an empty block - that is what makes older iOS accept the target on
    /// the first try instead of retrying for several refresh cycles.
    synthesized_payloads: Arc<Mutex<HashMap<String, Vec<u8>>>>,
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
            events_file: None,
            synthesized_payloads: Arc::new(Mutex::new(HashMap::new())),
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

    /// Serve one accepted client TCP connection: TLS terminate, run the H2
    /// server, and proxy each request to the upstream, patching WLOC
    /// responses. Returns once the client connection closes.
    pub async fn handle_connection(
        &self,
        client_tcp: TcpStream,
        patch: Option<&PatchTarget>,
    ) -> Result<(), MitmProxyError> {
        self.handle_connection_with_target(client_tcp, patch.copied())
            .await
    }

    /// Serve one connection and select its patch target from the original
    /// client source address. No default profile is used when resolution
    /// fails.
    pub async fn handle_connection_routed(
        &self,
        client_tcp: TcpStream,
        resolver: &dyn PatchTargetResolver,
    ) -> Result<(), MitmProxyError> {
        let source = client_tcp.peer_addr().ok().map(|address| address.ip());
        let patch = source.and_then(|address| resolver.resolve_patch_target(address));
        self.handle_connection_with_target(client_tcp, patch).await
    }

    async fn handle_connection_with_target(
        &self,
        client_tcp: TcpStream,
        patch: Option<PatchTarget>,
    ) -> Result<(), MitmProxyError> {
        let client_addr = client_tcp
            .peer_addr()
            .ok()
            .map(|addr| addr.ip().to_string())
            .unwrap_or_else(|| "-".to_owned());
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
            match self
                .forward_upstream(request, patch.as_ref(), &client_addr)
                .await
            {
                Ok((original_len, patched_body)) => {
                    self.append_rewrite_event(original_len, patched_body.len());
                    let mut send = respond
                        .send_response(Response::new(()), patched_body.is_empty())
                        .map_err(|error| MitmProxyError::H2(error.to_string()))?;
                    if !patched_body.is_empty() {
                        let _ = send.send_data(Bytes::from(patched_body), true);
                    }
                }
                Err(error) => {
                    eprintln!("wloc proxy: upstream failure: {error}");
                    break;
                }
            }
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
        client_addr: &str,
    ) -> Result<(usize, Vec<u8>), MitmProxyError> {
        let hostname = approved_host(&request)?;
        let is_wloc =
            request.uri().path() == WLOC_PATH || request.uri().path().ends_with("/clls/wloc");
        eprintln!(
            "wloc proxy: request host={hostname} method={} uri={} is_wloc={is_wloc}",
            request.method(),
            request.uri()
        );

        // Read the bounded client request body.
        let mut request_body = Vec::new();
        let (parts, mut client_body) = request.into_parts();
        while let Some(chunk) = client_body.data().await {
            let chunk = chunk.map_err(|error| MitmProxyError::H2(error.to_string()))?;
            request_body.extend_from_slice(&chunk);
            if request_body.len() > MAX_FORWARD_BODY_BYTES {
                return Err(MitmProxyError::Upstream(
                    "request body exceeds bound".into(),
                ));
            }
        }
        eprintln!("wloc proxy: request body {} bytes", request_body.len());

        // WLOC synthesis: when enabled, answer directly from the request
        // instead of forwarding to Apple (millisecond responses, like local
        // proxy apps). Set WLOC_SYNTH_RESPONSE=1 to activate.
        if is_wloc && std::env::var("WLOC_SYNTH_RESPONSE").as_deref() == Ok("1") {
            if let Some(target) = patch {
                let request_body = request_body.clone();
                match crate::wloc::synthesize_wloc_response(&request_body, target) {
                    Ok(patched) => {
                        let (kind, payload) =
                            crate::wloc::synthesized_parts(&patched).unwrap_or((1, &[][..]));
                        if payload.is_empty() {
                            // Coordinate query (kind 3): answer with the last
                            // known visible devices instead of an empty block,
                            // so the phone gets its APs at the target without
                            // waiting for the next BSSID round trip.
                            if let Some(cached) = self
                                .synthesized_payloads
                                .lock()
                                .ok()
                                .and_then(|cache| cache.get(client_addr).cloned())
                            {
                                let mut out = Vec::with_capacity(10 + cached.len());
                                out.extend([0x00, 0x01, 0x00, 0x00, 0x00, kind]);
                                out.extend((cached.len() as u32).to_be_bytes());
                                out.extend(cached);
                                eprintln!(
                                    "wloc proxy: synthesized kind={kind} from cache -> {} bytes",
                                    out.len()
                                );
                                return Ok((request_body.len(), out));
                            }
                        } else if let Ok(mut cache) = self.synthesized_payloads.lock() {
                            cache_synthesized_payload(&mut cache, client_addr, payload.to_vec());
                        }
                        eprintln!(
                            "wloc proxy: synthesized {} -> {} bytes (is_wloc={is_wloc})",
                            request_body.len(),
                            patched.len()
                        );
                        return Ok((request_body.len(), patched));
                    }
                    // Fail open: any synthesis error falls through to the
                    // upstream forwarding path below.
                    Err(_) => eprintln!("wloc proxy: synthesis failed, forwarding upstream"),
                }
            }
        }

        let (connect_host, connect_port) = match &self.upstream_override {
            Some((host, port)) => (host.clone(), *port),
            None => (hostname.clone(), 443),
        };
        let connect = TcpStream::connect((connect_host.as_str(), connect_port))
            .await
            .map_err(|error| MitmProxyError::Upstream(error.to_string()))?;
        let server_name = rustls::pki_types::ServerName::try_from(hostname.clone())
            .map_err(|_| MitmProxyError::Upstream("invalid upstream hostname".into()))?;
        let upstream_tls = self
            .upstream_connector
            .connect(server_name, connect)
            .await
            .map_err(|error| MitmProxyError::Upstream(error.to_string()))?;

        // The real Apple /clls/wloc endpoint serves HTTP/1.1; an h2 upstream
        // fails with "frame with invalid size". Forward over HTTP/1.1 and
        // decode Content-Length / chunked bodies.
        let upstream_request = sanitized_forward_request(parts, &hostname)?;
        let (_, _, body) =
            crate::mitm::http1::forward_http1(upstream_tls, &upstream_request, &request_body)
                .await?;

        let patched = maybe_patch_body(&body, is_wloc, patch);
        eprintln!(
            "wloc proxy: response body {} -> {} bytes (is_wloc={is_wloc}, patch={})",
            body.len(),
            patched.len(),
            patch.is_some()
        );
        // Debug aid: dump WLOC request/response samples so mismatched
        // response structures can be inspected on the device.
        if is_wloc && std::env::var("WLOC_DEBUG_DUMP").as_deref() == Ok("1") {
            if let Ok(dir) = std::env::var("WLOC_DEBUG_DUMP_DIR") {
                dump_wloc_samples(&dir, &hostname, client_addr, &request_body, &body, &patched);
            }
        }
        Ok((body.len(), patched))
    }

    /// Append one rewrite event per patched WLOC response.
    fn append_rewrite_event(&self, before: usize, after: usize) {
        let Some(events_file) = &self.events_file else {
            return;
        };
        if before == after {
            return;
        }
        let event = serde_json::json!({
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            "component": "wloc",
            "profile_scope": "device-policy",
            "severity": "info",
            "event_code": "response_rewritten",
            "message": "WLOC response rewritten",
            "fields": {
                "bytes_before": before,
                "bytes_after": after,
            },
        });
        append_json_line(events_file, &event);
    }
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
    if !crate::APPROVED_WLOC_HOSTS.contains(&hostname) {
        return Err(MitmProxyError::Upstream(format!(
            "host not approved: {hostname}"
        )));
    }
    Ok(hostname.to_owned())
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

/// Patch the response body if this is a `/clls/wloc` response; otherwise, or
/// on any patch failure, forward the original body unchanged (fail-open).
/// Write one explicitly enabled WLOC exchange for offline inspection.
///
/// This is never enabled by the production init script. The directory and
/// files are private, bounded, and created without following pre-existing
/// symlinks. Raw samples are intentionally excluded from normal diagnostics.
pub(crate) fn dump_wloc_samples(
    dir: &str,
    hostname: &str,
    client_addr: &str,
    request: &[u8],
    response: &[u8],
    patched: &[u8],
) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let directory = std::path::Path::new(dir);
    if !directory.is_absolute()
        || directory
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return;
    }
    if let Ok(metadata) = std::fs::symlink_metadata(directory) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return;
        }
    }
    if std::fs::create_dir_all(directory).is_err() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700));
    }
    let safe_host = safe_file_token(hostname);
    let safe_client = opaque_client_token(client_addr);
    write_bounded_sample(
        &directory.join(format!("{stamp}_{safe_client}_{safe_host}_req.bin")),
        request,
    );
    write_bounded_sample(
        &directory.join(format!("{stamp}_{safe_client}_{safe_host}_resp.bin")),
        response,
    );
    write_bounded_sample(
        &directory.join(format!("{stamp}_{safe_client}_{safe_host}_patched.bin")),
        patched,
    );
}

fn safe_file_token(value: &str) -> String {
    let token: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    if token.is_empty() {
        "unknown".to_owned()
    } else {
        token
    }
}

fn opaque_client_token(value: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, value.as_bytes());
    digest
        .as_ref()
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_bounded_sample(path: &std::path::Path, bytes: &[u8]) {
    let end = bytes.len().min(MAX_DEBUG_SAMPLE_BYTES);
    use std::io::Write as _;
    let Ok(mut file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    else {
        return;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
    let _ = file.write_all(&bytes[..end]);
}

fn maybe_patch_body(body: &[u8], is_wloc: bool, patch: Option<&PatchTarget>) -> Vec<u8> {
    match patch {
        Some(patch) if is_wloc => patch_wloc_response(body, patch),
        _ => body.to_vec(),
    }
}

fn cache_synthesized_payload(
    cache: &mut HashMap<String, Vec<u8>>,
    client_addr: &str,
    payload: Vec<u8>,
) {
    if payload.is_empty() || payload.len() > MAX_SYNTHESIZED_PAYLOAD_BYTES {
        return;
    }
    cache.remove(client_addr);
    while cache.len() >= MAX_SYNTHESIZED_CLIENTS
        || cache
            .values()
            .map(Vec::len)
            .sum::<usize>()
            .saturating_add(payload.len())
            > MAX_SYNTHESIZED_CACHE_BYTES
    {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        cache.remove(&oldest);
    }
    cache.insert(client_addr.to_owned(), payload);
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
    fn synthesized_client_cache_has_entry_and_byte_bounds() {
        let mut cache = HashMap::new();
        for index in 0..(MAX_SYNTHESIZED_CLIENTS * 2) {
            cache_synthesized_payload(
                &mut cache,
                &format!("192.0.2.{index}"),
                vec![0_u8; MAX_SYNTHESIZED_PAYLOAD_BYTES / 2],
            );
        }
        assert!(cache.len() <= MAX_SYNTHESIZED_CLIENTS);
        assert!(cache.values().map(Vec::len).sum::<usize>() <= MAX_SYNTHESIZED_CACHE_BYTES);

        cache_synthesized_payload(
            &mut cache,
            "oversized",
            vec![0_u8; MAX_SYNTHESIZED_PAYLOAD_BYTES + 1],
        );
        assert!(!cache.contains_key("oversized"));
    }

    #[test]
    fn dump_wloc_samples_writes_three_files() {
        let dir = std::env::temp_dir().join(format!("wloc-dump-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dump_wloc_samples(
            dir.to_str().unwrap(),
            "gs-loc-cn.apple.com",
            "192.168.31.175",
            b"request-body",
            b"response-body",
            b"patched-body",
        );
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 3);
        let names: Vec<String> = entries
            .iter()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().any(|n| n.ends_with("_req.bin")));
        assert!(names.iter().any(|n| n.ends_with("_resp.bin")));
        assert!(names.iter().any(|n| n.ends_with("_patched.bin")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn debug_samples_are_bounded() {
        let dir = std::env::temp_dir().join(format!("wloc-dump-bound-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let large = vec![b'x'; MAX_DEBUG_SAMPLE_BYTES * 2];
        dump_wloc_samples(
            dir.to_str().unwrap(),
            "gs-loc.apple.com",
            "192.168.31.175",
            &large,
            &large,
            &large,
        );
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            assert!(!entry
                .file_name()
                .to_string_lossy()
                .contains("192.168.31.175"));
            assert!(entry.metadata().unwrap().len() <= MAX_DEBUG_SAMPLE_BYTES as u64);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
