//! CA and leaf certificate lifecycle for the approved WLOC hosts.
//!
//! Generates an in-memory root CA and per-host leaf certificates at runtime on
//! the router; the private keys are never persisted, exported, or committed.
//! The rustls resolver serves a leaf only for the six approved WLOC hostnames
//! (fail-closed), so no other domain can ever be impersonated.

pub mod http1;
pub mod proxy;

use std::collections::HashMap;
use std::sync::Arc;

use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair, KeyUsagePurpose,
};
use rustls::client::danger::ServerCertVerifier;
use rustls::crypto::ring::sign::any_supported_type;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey as RustlsCertifiedKey;
use rustls::RootCertStore;

use crate::APPROVED_WLOC_HOSTS;

/// Failure modes for CA and leaf lifecycle operations.
#[derive(Clone, Debug)]
pub enum MitmError {
    /// Certificate generation or signing failed.
    Certificate(String),
    /// A leaf was requested for a hostname outside the approved set.
    HostNotApproved(String),
}

impl std::fmt::Display for MitmError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Certificate(message) => write!(formatter, "certificate error: {message}"),
            Self::HostNotApproved(host) => write!(formatter, "host not approved for MITM: {host}"),
        }
    }
}

impl std::error::Error for MitmError {}

impl From<rcgen::Error> for MitmError {
    fn from(error: rcgen::Error) -> Self {
        Self::Certificate(error.to_string())
    }
}

/// A generated root CA with its private key held only in memory.
pub struct CaBundle {
    root_params: CertificateParams,
    root_cert_der: Vec<u8>,
    root_key: KeyPair,
}

impl CaBundle {
    /// Generate a fresh root CA. Keys exist only in memory for the lifetime of
    /// this object unless explicitly exported for on-device persistence.
    pub fn generate() -> Result<Self, MitmError> {
        let params = Self::ca_params()?;
        let root_key = KeyPair::generate()?;
        let root_cert = params.self_signed(&root_key)?;
        Ok(Self {
            root_params: params,
            root_cert_der: root_cert.der().to_vec(),
            root_key,
        })
    }

    /// Load a persisted root CA from its private key (PKCS#8 DER) and
    /// certificate (DER). The private key must come from root-only storage on
    /// the device; it is never written to the repository. The issuer
    /// parameters are reconstructed to match `generate()` so issued leaves
    /// chain to the same root certificate and key.
    pub fn load(key_der: &[u8], cert_der: &[u8]) -> Result<Self, MitmError> {
        use rcgen::PKCS_ECDSA_P256_SHA256;
        use rustls::pki_types::PrivatePkcs8KeyDer;
        let root_key = KeyPair::from_pkcs8_der_and_sign_algo(
            &PrivatePkcs8KeyDer::from(key_der.to_vec()),
            &PKCS_ECDSA_P256_SHA256,
        )
        .map_err(MitmError::from)?;
        let root_params = Self::ca_params()?;
        Ok(Self {
            root_params,
            root_cert_der: cert_der.to_vec(),
            root_key,
        })
    }

    /// The issuer parameters shared by `generate()` and `load()` so leaf
    /// chains validate against both paths.
    fn ca_params() -> Result<CertificateParams, MitmError> {
        let mut params = CertificateParams::new(Vec::<String>::new())?;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, "wloc-service root CA");
        params.distinguished_name = distinguished_name;
        // A fixed, explicit lifetime (10 years) so the admin UI can show the
        // expiry date; rcgen's default spans centuries.
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::days(1);
        params.not_after = now + time::Duration::days(3650);
        Ok(params)
    }

    /// SHA-256 fingerprint of the root certificate, colon-separated hex.
    pub fn fingerprint_sha256(&self) -> String {
        let digest = ring::digest::digest(&ring::digest::SHA256, &self.root_cert_der);
        digest
            .as_ref()
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }

    /// Expiry of the root certificate as a unix timestamp.
    pub fn not_after_unix(&self) -> i64 {
        self.root_params.not_after.unix_timestamp()
    }

    /// Export the root private key (PKCS#8 DER) and certificate (DER) for
    /// on-device persistence. The caller must store the key with root-only
    /// permissions and never commit it.
    pub fn export_key_der(&self) -> Vec<u8> {
        self.root_key.serialize_der()
    }

    /// The root CA certificate in DER form, for installation on the test
    /// device as a trusted root. Contains no private key material.
    pub fn root_cert_der(&self) -> CertificateDer<'static> {
        CertificateDer::from(self.root_cert_der.clone())
    }

    /// Build a rustls root store containing this CA (used to verify leaf
    /// chains and to configure upstream verification semantics).
    pub fn root_store(&self) -> Result<RootCertStore, MitmError> {
        let mut store = RootCertStore::empty();
        store
            .add(self.root_cert_der())
            .map_err(|error| MitmError::Certificate(error.to_string()))?;
        Ok(store)
    }

    /// Issue a server-auth leaf certificate for `hostname`. **Fail-closed**:
    /// only the six approved WLOC hostnames can be issued.
    pub fn issue_leaf(&self, hostname: &str) -> Result<LeafCertificate, MitmError> {
        if !APPROVED_WLOC_HOSTS.contains(&hostname) {
            return Err(MitmError::HostNotApproved(hostname.to_owned()));
        }
        let mut params = CertificateParams::new(vec![hostname.to_owned()])?;
        params.is_ca = IsCa::NoCa;
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, hostname);
        params.distinguished_name = distinguished_name;

        let leaf_key = KeyPair::generate()?;
        let issuer = Issuer::from_params(&self.root_params, &self.root_key);
        let leaf_cert = params.signed_by(&leaf_key, &issuer)?;
        LeafCertificate::new(hostname, leaf_cert, leaf_key)
    }
}

/// A signed leaf certificate for one approved host, converted to a rustls
/// [`RustlsCertifiedKey`] for the TLS acceptor.
pub struct LeafCertificate {
    hostname: String,
    certified_key: RustlsCertifiedKey,
}

impl LeafCertificate {
    fn new(hostname: &str, cert: Certificate, key: KeyPair) -> Result<Self, MitmError> {
        let signing_key = any_supported_type(&PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
            key.serialize_der(),
        )))
        .map_err(|error| MitmError::Certificate(error.to_string()))?;
        let certified_key =
            RustlsCertifiedKey::new(vec![CertificateDer::from(cert.der().to_vec())], signing_key);
        Ok(Self {
            hostname: hostname.to_owned(),
            certified_key,
        })
    }

    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    pub fn certified_key(&self) -> &RustlsCertifiedKey {
        &self.certified_key
    }

    /// The leaf certificate DER, for tests and diagnostics.
    pub fn cert_der(&self) -> &CertificateDer<'static> {
        &self.certified_key.cert[0]
    }
}

/// A rustls cert resolver that serves the issued leaf for the approved
/// hostname in the ClientHello and refuses everything else (fail-closed).
pub struct MitmCertResolver {
    leaves: HashMap<String, Arc<RustlsCertifiedKey>>,
}

impl MitmCertResolver {
    pub fn new(ca: &CaBundle) -> Result<Self, MitmError> {
        let mut leaves = HashMap::new();
        for hostname in APPROVED_WLOC_HOSTS {
            let leaf = ca.issue_leaf(hostname)?;
            leaves.insert(hostname.to_owned(), Arc::new(leaf.certified_key().clone()));
        }
        Ok(Self { leaves })
    }
}

impl ResolvesServerCert for MitmCertResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<RustlsCertifiedKey>> {
        let server_name = client_hello.server_name()?;
        // rustls gives the SNI without a trailing dot; normalize before lookup.
        let hostname = server_name.trim_end_matches('.');
        self.leaves.get(hostname).cloned()
    }
}

impl std::fmt::Debug for MitmCertResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MitmCertResolver")
            .field("hosts", &self.leaves.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// Verify that a leaf chain for `hostname` validates against `ca`.
pub fn verify_leaf(ca: &CaBundle, leaf: &LeafCertificate, hostname: &str) -> Result<(), MitmError> {
    let roots = ca.root_store()?;
    let server_name = ServerName::try_from(hostname.to_owned())
        .map_err(|error| MitmError::Certificate(error.to_string()))?;
    let verifier = rustls::client::WebPkiServerVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|error| MitmError::Certificate(error.to_string()))?;
    verifier
        .verify_server_cert(leaf.cert_der(), &[], &server_name, &[], UnixTime::now())
        .map(|_| ())
        .map_err(|error| MitmError::Certificate(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::APPROVED_WLOC_HOSTS;

    #[test]
    fn root_ca_der_is_exportable_without_private_key() {
        let ca = CaBundle::generate().unwrap();
        let der = ca.root_cert_der();
        assert!(!der.as_ref().is_empty());
        // The DER must parse as a certificate via rustls.
        let mut store = RootCertStore::empty();
        store.add(der).expect("root DER must parse");
    }

    #[test]
    fn persisted_ca_round_trips_and_issues_valid_leaves() {
        let ca = CaBundle::generate().unwrap();
        let key_der = ca.export_key_der();
        let cert_der = ca.root_cert_der().as_ref().to_vec();

        let loaded = CaBundle::load(&key_der, &cert_der).unwrap();
        // The loaded CA exposes the same root certificate.
        assert_eq!(loaded.root_cert_der().as_ref(), cert_der.as_slice());
        // It can issue leaves that chain to the persisted root, so iPhone
        // trust survives a daemon restart that reloads the CA.
        let hostname = APPROVED_WLOC_HOSTS[0];
        let leaf = loaded.issue_leaf(hostname).unwrap();
        verify_leaf(&loaded, &leaf, hostname).expect("loaded CA leaf must validate");
    }

    #[test]
    fn leaf_is_issued_only_for_approved_hosts() {
        let ca = CaBundle::generate().unwrap();
        for hostname in APPROVED_WLOC_HOSTS {
            let leaf = ca.issue_leaf(hostname).unwrap();
            assert_eq!(leaf.hostname(), hostname);
        }
        for hostname in [
            "www.apple.com",
            "evil.example.org",
            "gs-loc.apple.com.evil.org",
        ] {
            assert!(matches!(
                ca.issue_leaf(hostname),
                Err(MitmError::HostNotApproved(_))
            ));
        }
    }

    #[test]
    fn issued_leaf_chains_to_the_root_for_the_hostname() {
        let ca = CaBundle::generate().unwrap();
        let hostname = APPROVED_WLOC_HOSTS[0];
        let leaf = ca.issue_leaf(hostname).unwrap();
        verify_leaf(&ca, &leaf, hostname).expect("leaf must validate against the CA");
    }

    #[test]
    fn leaf_for_a_different_hostname_fails_verification() {
        let ca = CaBundle::generate().unwrap();
        let leaf = ca.issue_leaf(APPROVED_WLOC_HOSTS[0]).unwrap();
        // The leaf is signed for host 0, so verifying it as host 1 fails.
        assert!(verify_leaf(&ca, &leaf, APPROVED_WLOC_HOSTS[1]).is_err());
    }

    #[test]
    fn resolver_serves_leaves_only_for_approved_hostnames() {
        let ca = CaBundle::generate().unwrap();
        let resolver = MitmCertResolver::new(&ca).unwrap();
        for hostname in APPROVED_WLOC_HOSTS {
            assert!(
                resolver.leaves.contains_key(hostname),
                "resolver must hold a leaf for {hostname}"
            );
        }
    }
}
