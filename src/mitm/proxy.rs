//! HTTP/2 MITM proxy core.
//!
//! Terminates TLS on the client side (approved host only via the certificate
//! resolver), bridges HTTP/2 to the real upstream (verifying its certificate),
//! and intercepts the `/clls/wloc` response path to apply the WLOC patch.
//!
//! Fail-open: any interception error forwards the original upstream response
//! unchanged; a malformed WLOC response is never replaced.

use std::sync::Arc;

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

/// HTTP/2 MITM proxy bound to one approved hostname's traffic.
#[derive(Clone)]
pub struct MitmProxy {
    tls_config: Arc<rustls::ServerConfig>,
    upstream_connector: TlsConnector,
    /// Test hook: override the TCP connect target while keeping the approved
    /// hostname for SNI and the Host header. Production uses hostname:443.
    upstream_override: Option<(String, u16)>,
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
        client.alpn_protocols = vec![b"h2".to_vec()];

        Ok(Self {
            tls_config: Arc::new(server),
            upstream_connector: TlsConnector::from(Arc::new(client)),
            upstream_override: None,
        })
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
            match self.forward_upstream(request, patch).await {
                Ok(patched_body) => {
                    let mut send = respond
                        .send_response(Response::new(()), patched_body.is_empty())
                        .map_err(|error| MitmProxyError::H2(error.to_string()))?;
                    if !patched_body.is_empty() {
                        let _ = send.send_data(Bytes::from(patched_body), true);
                    }
                }
                Err(_) => break,
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
    ) -> Result<Vec<u8>, MitmProxyError> {
        let hostname = approved_host(&request)?;
        let is_wloc =
            request.uri().path() == WLOC_PATH || request.uri().path().ends_with("/clls/wloc");

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

        let (mut send_request, connection) = h2::client::Builder::new()
            .initial_window_size(32 * 1024)
            .max_frame_size(16 * 1024)
            .handshake::<_, Bytes>(upstream_tls)
            .await
            .map_err(|error| MitmProxyError::H2(error.to_string()))?;
        let driver = tokio::spawn(async move {
            let _ = connection.await;
        });

        let upstream_request = sanitized_forward_request(parts, &hostname)?;
        let (response_future, mut send_stream) = send_request
            .send_request(upstream_request, request_body.is_empty())
            .map_err(|error| MitmProxyError::H2(error.to_string()))?;
        if !request_body.is_empty() {
            send_stream
                .send_data(Bytes::from(request_body), true)
                .map_err(|error| MitmProxyError::H2(error.to_string()))?;
        }
        let mut response = response_future
            .await
            .map_err(|error| MitmProxyError::Upstream(error.to_string()))?;

        let mut body = Vec::new();
        while let Some(chunk) = response.body_mut().data().await {
            let chunk = chunk.map_err(|error| MitmProxyError::Upstream(error.to_string()))?;
            body.extend_from_slice(&chunk);
            if body.len() > MAX_FORWARD_BODY_BYTES {
                driver.abort();
                return Err(MitmProxyError::Upstream(
                    "response body exceeds bound".into(),
                ));
            }
        }
        driver.abort();
        let _ = driver.await;
        Ok(maybe_patch_body(&body, is_wloc, patch))
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
