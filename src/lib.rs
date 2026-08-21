use std::sync::Arc;

use prost::Message;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use tokio_rustls::{TlsAcceptor, TlsConnector};

pub mod app;
pub mod config;
pub(crate) mod diagnostics;
pub mod exitprobe;
pub mod georesolver;
pub mod mitm;
pub mod runtime;
pub mod service;
pub mod tls_h2;
pub mod wloc;

/// Approved WLOC hostnames whose traffic may be intercepted. The scope is
/// deliberately limited to the two exact Apple names required by the
/// OpenWrt traffic-isolation contract; DNS CNAME targets are not interception
/// hostnames.
pub const APPROVED_WLOC_HOSTS: [&str; 2] = ["gs-loc.apple.com", "gs-loc-cn.apple.com"];
pub const MAX_WLOC_BODY_BYTES: u64 = 512 * 1024;
const MIN_H2_FRAME_SIZE: u32 = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateMeta {
    pub source_ip: String,
    pub hostname: String,
    pub content_length: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateDecision {
    Candidate,
    PassThrough,
}

#[derive(Clone, PartialEq, Message)]
pub struct SyntheticProbeEnvelope {
    #[prost(bytes = "vec", tag = "1")]
    pub payload: Vec<u8>,
}

#[derive(Debug)]
struct NullCertificateResolver;

impl ResolvesServerCert for NullCertificateResolver {
    fn resolve(&self, _client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        None
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct TlsStackReport {
    pub server_configured: bool,
    pub upstream_verification_configured: bool,
    pub server_alpn_protocols: usize,
    pub client_alpn_protocols: usize,
}

pub struct TlsStack {
    pub acceptor: TlsAcceptor,
    pub connector: TlsConnector,
    pub report: TlsStackReport,
}

pub fn inspect_candidate(meta: &CandidateMeta) -> GateDecision {
    if !APPROVED_WLOC_HOSTS.contains(&meta.hostname.as_str()) {
        return GateDecision::PassThrough;
    }

    match meta.content_length {
        Some(length) if length > 0 && length <= MAX_WLOC_BODY_BYTES => GateDecision::Candidate,
        _ => GateDecision::PassThrough,
    }
}

pub fn roundtrip_synthetic_probe() -> Result<SyntheticProbeEnvelope, prost::DecodeError> {
    let envelope = SyntheticProbeEnvelope {
        payload: vec![1, 2, 3, 4, 5],
    };

    let mut bytes = Vec::with_capacity(envelope.encoded_len());
    envelope.encode(&mut bytes).expect("Vec encode cannot fail");
    SyntheticProbeEnvelope::decode(bytes.as_slice())
}

pub fn build_tls_stack() -> Result<TlsStack, rustls::Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let versions = [&rustls::version::TLS13, &rustls::version::TLS12];

    let mut server = rustls::ServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&versions)?
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(NullCertificateResolver));
    server.alpn_protocols = vec![b"h2".to_vec()];

    // An empty root store is deliberately fail-closed for the spike. Production
    // must load an audited trust store; it must never disable verification.
    let mut client = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&versions)?
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();
    client.alpn_protocols = vec![b"h2".to_vec()];

    let report = TlsStackReport {
        server_configured: true,
        upstream_verification_configured: true,
        server_alpn_protocols: server.alpn_protocols.len(),
        client_alpn_protocols: client.alpn_protocols.len(),
    };

    Ok(TlsStack {
        acceptor: TlsAcceptor::from(Arc::new(server)),
        connector: TlsConnector::from(Arc::new(client)),
        report,
    })
}

pub async fn run_h2_prior_knowledge_smoke() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    let (client_io, server_io) = tokio::io::duplex(4096);

    let server = tokio::spawn(async move {
        h2::server::Builder::new()
            .initial_window_size(32 * 1024)
            .max_frame_size(MIN_H2_FRAME_SIZE)
            .max_concurrent_streams(1)
            .handshake::<_, bytes::Bytes>(server_io)
            .await
            .map(|_connection| ())
    });

    let (sender, connection) = h2::client::Builder::new()
        .initial_window_size(32 * 1024)
        .max_frame_size(MIN_H2_FRAME_SIZE)
        .handshake::<_, bytes::Bytes>(client_io)
        .await?;
    let client_driver = tokio::spawn(connection);

    server.await??;
    drop(sender);
    client_driver.abort();
    let _ = client_driver.await;
    Ok(())
}
