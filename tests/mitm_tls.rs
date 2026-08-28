//! MITM TLS handshake contract: approved host completes with h2 ALPN, and a
//! non-approved SNI is refused (fail-closed) by the certificate resolver.

use std::sync::Arc;

use rustls::pki_types::ServerName;
use tokio_rustls::{TlsAcceptor, TlsConnector};

use wificalling_location_gateway::mitm::{CaBundle, MitmCertResolver};

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn server_config(ca: &CaBundle) -> rustls::ServerConfig {
    let resolver = MitmCertResolver::new(ca).expect("resolver builds");
    let mut config = rustls::ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS 1.3 versions")
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(resolver));
    config.alpn_protocols = vec![b"h2".to_vec()];
    config
}

fn client_config(ca: &CaBundle) -> rustls::ClientConfig {
    let roots = ca.root_store().expect("root store builds");
    let mut config = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS 1.3 versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    config
}

async fn handshake(
    server_config: rustls::ServerConfig,
    client_config: rustls::ClientConfig,
    hostname: &str,
) -> Result<(bool, Option<Vec<u8>>), std::io::Error> {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);

    let server_task = tokio::spawn(async move {
        let acceptor = TlsAcceptor::from(Arc::new(server_config));
        acceptor.accept(server_io).await
    });

    let connector = TlsConnector::from(Arc::new(client_config));
    let server_name = ServerName::try_from(hostname.to_owned()).expect("valid hostname");
    let client_result = connector.connect(server_name, client_io).await;

    let server_result = server_task.await.expect("server task completes");
    match (client_result, server_result) {
        (Ok(client_stream), Ok(server_stream)) => {
            let client_alpn = client_stream.get_ref().1.alpn_protocol().map(Vec::from);
            let server_alpn = server_stream.get_ref().1.alpn_protocol().map(Vec::from);
            Ok((true, client_alpn.or(server_alpn)))
        }
        _ => Ok((false, None)),
    }
}

#[tokio::test]
async fn approved_host_completes_handshake_with_h2_alpn() {
    let ca = CaBundle::generate().unwrap();
    let (success, alpn) = handshake(server_config(&ca), client_config(&ca), "gs-loc.apple.com")
        .await
        .unwrap();
    assert!(success, "approved host must complete the TLS handshake");
    assert_eq!(alpn, Some(b"h2".to_vec()), "h2 ALPN must be negotiated");
}

#[tokio::test]
async fn all_approved_hosts_complete_handshake() {
    let ca = CaBundle::generate().unwrap();
    for hostname in [
        "gs-loc.apple.com",
        "gs-loc-cn.apple.com",
        "gsp-ssl.ls.apple.com",
        "bluedot.is.autonavi.com",
        "bluedot.is.autonavi.com.gds.alibabadns.com",
        "gspe19-cn-ssl-ls-apple-com.v.aaplimg.com",
    ] {
        let (success, _) = handshake(server_config(&ca), client_config(&ca), hostname)
            .await
            .unwrap();
        assert!(
            success,
            "approved host {hostname} must complete the handshake"
        );
    }
}

#[tokio::test]
async fn approved_host_sni_matching_is_case_insensitive() {
    let ca = CaBundle::generate().unwrap();
    let (success, _) = handshake(server_config(&ca), client_config(&ca), "GS-LOC.APPLE.COM.")
        .await
        .unwrap();
    assert!(
        success,
        "DNS case and a root dot must not break an approved SNI"
    );
}

#[tokio::test]
async fn non_approved_host_is_refused_fail_closed() {
    let ca = CaBundle::generate().unwrap();
    for hostname in [
        "www.apple.com",
        "google.com",
        "gs-loc.apple.com.evil.org",
        "evil-gs-loc.apple.com",
    ] {
        let (success, _) = handshake(server_config(&ca), client_config(&ca), hostname)
            .await
            .unwrap();
        assert!(!success, "host {hostname} must be refused (fail-closed)");
    }
}

#[tokio::test]
async fn client_without_the_ca_trust_is_refused() {
    let ca = CaBundle::generate().unwrap();
    // A client that trusts no roots cannot validate the leaf.
    let mut untrusted = rustls::ClientConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS 1.3 versions")
        .with_root_certificates(rustls::RootCertStore::empty())
        .with_no_client_auth();
    untrusted.alpn_protocols = vec![b"h2".to_vec()];

    let (success, _) = handshake(server_config(&ca), untrusted, "gs-loc.apple.com")
        .await
        .unwrap();
    assert!(
        !success,
        "a client that does not trust the CA must be refused"
    );
}
