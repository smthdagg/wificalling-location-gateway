//! Run the WLOC service control daemon over a root-owned Unix socket.
//!
//! The daemon serves the frozen control API on a local Unix socket. Runtime
//! adapters are stubs until the OpenWrt sing-box/nftables/Geo adapters land;
//! their behavior is configurable through the `WLOC_STUB_*` environment
//! variables so the control plane can be exercised end to end on any host.
//!
//! Socket path: `WLOC_SOCKET` (default `/var/run/wloc-service/control.sock`).

use std::error::Error;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wificalling_location_gateway::app::{WlocService, WlocServiceConfig};
use wificalling_location_gateway::exitprobe::runtime::{ExitProbeRuntime, ProbeFailure};
use wificalling_location_gateway::exitprobe::{NodeRef, ProbeLimits};
use wificalling_location_gateway::georesolver::http::GeoHttpClient;
use wificalling_location_gateway::georesolver::runtime::{GeoProviderRuntime, ProviderFailure};
use wificalling_location_gateway::georesolver::ProviderRef;
use wificalling_location_gateway::mitm::proxy::MitmProxy;
use wificalling_location_gateway::mitm::CaBundle;
use wificalling_location_gateway::service::control::{RuntimeControl, RuntimeFailure};
use wificalling_location_gateway::service::server::ControlServer;
use wificalling_location_gateway::service::GeoRecord;
use wificalling_location_gateway::wloc::PatchTarget;

fn env_or<T>(name: &str, default: T) -> T
where
    T: std::str::FromStr,
{
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// No-op runtime control: every adapter step succeeds and the engine is
/// healthy. Replaced by the nftables/procd adapter on OpenWrt.
struct StubRuntime;

impl RuntimeControl for StubRuntime {
    fn start_engine_passthrough(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn engine_healthy(&mut self) -> Result<bool, RuntimeFailure> {
        Ok(true)
    }
    fn arm_watchdog(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn install_exact_redirect(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn remove_redirect(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn redirect_present(&mut self) -> Result<bool, RuntimeFailure> {
        Ok(false)
    }
    fn disarm_watchdog(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn drain_engine(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn stop_engine(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
}

/// Stub exit probe: reports the configured exit and WAN addresses.
struct StubProbe {
    exit_ip: IpAddr,
    wan_ip: IpAddr,
}

impl ExitProbeRuntime for StubProbe {
    fn probe_exit_ip(&mut self) -> Result<IpAddr, ProbeFailure> {
        Ok(self.exit_ip)
    }
    fn router_wan_ips(&mut self) -> Result<Vec<IpAddr>, ProbeFailure> {
        Ok(vec![self.wan_ip])
    }
}

/// Build the exit probe: the real sing-box probe by default (follows the
/// device's bound node), or the deterministic stub when `WLOC_PROBE=stub`.
fn build_probe() -> Box<dyn ExitProbeRuntime> {
    if std::env::var("WLOC_PROBE").as_deref() == Ok("stub") {
        return Box::new(StubProbe {
            exit_ip: env_or("WLOC_STUB_EXIT_IP", IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))),
            wan_ip: env_or("WLOC_STUB_WAN_IP", IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))),
        });
    }
    let device_ip: IpAddr = env_or(
        "WLOC_DEVICE_IP",
        IpAddr::V4(Ipv4Addr::new(192, 168, 31, 176)),
    );
    Box::new(
        wificalling_location_gateway::exitprobe::singbox::SingBoxProbe::new(
            std::path::PathBuf::from(
                std::env::var("WLOC_SINGBOX_CONFIG")
                    .unwrap_or_else(|_| "/var/run/wificalling-gateway/sing-box.json".into()),
            ),
            device_ip,
            env_or("WLOC_PROBE_PORT", 18080_u16),
            std::path::PathBuf::from("/tmp/wloc-probe"),
        ),
    )
}

/// Stub Geo provider: returns a fixed, valid record for the queried exit.
struct StubGeo {
    country_code: String,
    latitude: f64,
    longitude: f64,
}

impl GeoProviderRuntime for StubGeo {
    fn lookup(
        &mut self,
        _provider: ProviderRef,
        ip: IpAddr,
    ) -> Result<Option<(IpAddr, GeoRecord)>, ProviderFailure> {
        Ok(Some((
            ip,
            GeoRecord {
                country_code: self.country_code.clone(),
                latitude: self.latitude,
                longitude: self.longitude,
                timezone: "UTC".to_owned(),
                expires_at_unix: now_unix() + 3_600,
            },
        )))
    }
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let socket_path = std::env::var("WLOC_SOCKET")
        .unwrap_or_else(|_| "/var/run/wloc-service/control.sock".into());

    // Default to the real Geo HTTP provider; WLOC_GEO_PROVIDER=stub forces
    // the deterministic stub for offline development.
    let geo_provider: String = std::env::var("WLOC_GEO_PROVIDER").unwrap_or_else(|_| "http".into());
    let geo: Box<dyn GeoProviderRuntime> = if geo_provider == "stub" {
        Box::new(StubGeo {
            country_code: env_or("WLOC_STUB_COUNTRY", "US".to_owned()),
            latitude: env_or("WLOC_STUB_LAT", 37.77_f64),
            longitude: env_or("WLOC_STUB_LON", -122.41_f64),
        })
    } else {
        Box::new(GeoHttpClient::ip_api_default())
    };

    let service = WlocService::new(
        StubRuntime,
        build_probe(),
        geo,
        WlocServiceConfig {
            node_ref: NodeRef::new("default").expect("static node ref is valid"),
            providers: vec![ProviderRef::new("stub").expect("static provider ref is valid")],
            probe_limits: ProbeLimits {
                max_observation_age: Duration::from_secs(300),
            },
            scope_valid: true,
            ipv6_ready: true,
            assigned_device_configured: true,
        },
    );

    // MITM proxy: load the persisted root CA or generate one and persist it.
    // The private key stays in root-only on-device storage so iPhone trust
    // survives daemon restarts; the key is never written to the repository.
    let ca_path =
        std::env::var("WLOC_CA_CERT").unwrap_or_else(|_| "/var/run/wloc-service/ca.pem".into());
    let ca_key_path =
        std::env::var("WLOC_CA_KEY").unwrap_or_else(|_| "/var/run/wloc-service/ca.key".into());
    if let Some(parent) = Path::new(&ca_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mitm_ca = if Path::new(&ca_key_path).exists() && Path::new(&ca_path).exists() {
        let key_der = std::fs::read(&ca_key_path)?;
        let cert_der = pem_decode(&std::fs::read(&ca_path)?)?;
        CaBundle::load(&key_der, &cert_der)?
    } else {
        let ca = CaBundle::generate()?;
        std::fs::write(&ca_key_path, ca.export_key_der())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&ca_key_path, std::fs::Permissions::from_mode(0o600))?;
        }
        let ca_der = ca.root_cert_der();
        std::fs::write(&ca_path, pem_encode(&ca_der))?;
        eprintln!("MITM root CA generated; export at {ca_path} (install on the test device)");
        ca
    };
    eprintln!("MITM root CA ready (private key: {ca_key_path})");

    let mut upstream_roots = rustls::RootCertStore::empty();
    upstream_roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let proxy = MitmProxy::new(&mitm_ca, upstream_roots)?;
    let proxy_port: u16 = env_or("WLOC_PROXY_PORT", 8443_u16);

    let patch_state = std::sync::Arc::new(std::sync::Mutex::new(None::<PatchTarget>));
    let service = service.with_patch_sink(std::sync::Arc::clone(&patch_state));

    if let Some(parent) = Path::new(&socket_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&socket_path);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let listener = runtime.block_on(async { tokio::net::UnixListener::bind(&socket_path) })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    }

    let proxy_listener =
        runtime.block_on(tokio::net::TcpListener::bind(("0.0.0.0", proxy_port)))?;
    runtime.spawn(async move {
        loop {
            if let Ok((stream, _)) = proxy_listener.accept().await {
                let proxy = proxy.clone();
                let patch_state = std::sync::Arc::clone(&patch_state);
                tokio::spawn(async move {
                    let patch = patch_state.lock().ok().and_then(|guard| *guard);
                    let _ = proxy.handle_connection(stream, patch.as_ref()).await;
                });
            }
        }
    });
    eprintln!("wloc-service MITM proxy listening on 0.0.0.0:{proxy_port}");

    eprintln!("wloc-service listening on {socket_path}");
    let server = ControlServer::new(service);
    runtime.block_on(server.serve(listener));
    Ok(())
}

/// PEM-encode a DER certificate for installation on an iOS device.
fn pem_encode(der: &rustls::pki_types::CertificateDer<'_>) -> String {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(der.as_ref());
    let mut pem = String::from("-----BEGIN CERTIFICATE-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(chunk).expect("base64 is UTF-8"));
        pem.push('\n');
    }
    pem.push_str("-----END CERTIFICATE-----\n");
    pem
}

/// Decode a single PEM certificate block into DER.
fn pem_decode(pem: &[u8]) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    use base64::Engine;
    let text = std::str::from_utf8(pem)?;
    let body = text
        .lines()
        .skip_while(|line| !line.starts_with("-----BEGIN CERTIFICATE-----"))
        .skip(1)
        .take_while(|line| !line.starts_with("-----END CERTIFICATE-----"))
        .collect::<String>();
    Ok(base64::engine::general_purpose::STANDARD.decode(body)?)
}
