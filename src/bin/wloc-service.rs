//! Run the WLOC service control daemon over a root-owned Unix socket.
//!
//! The daemon serves the frozen control API on a local Unix socket. The
//! control plane owns the named WLOC nftables lifecycle while the proxy and
//! control API stay in one process.
//!
//! Socket path: `WLOC_SOCKET` (default `/var/run/wloc-service/control.sock`).

use std::error::Error;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::net::TcpStream;
use wificalling_location_gateway::app::{WlocService, WlocServiceConfig};
use wificalling_location_gateway::config::{LocationMode, WlocUciConfig};
use wificalling_location_gateway::exitprobe::runtime::{ExitProbeRuntime, ProbeFailure};
use wificalling_location_gateway::exitprobe::{NodeRef, ProbeLimits};
use wificalling_location_gateway::georesolver::http::GeoHttpClient;
use wificalling_location_gateway::georesolver::runtime::{GeoProviderRuntime, ProviderFailure};
use wificalling_location_gateway::georesolver::ProviderRef;
use wificalling_location_gateway::mitm::proxy::{MitmProxy, MitmProxyError};
use wificalling_location_gateway::mitm::CaBundle;
use wificalling_location_gateway::service::api::RequestParams;
use wificalling_location_gateway::service::control::{RuntimeControl, RuntimeFailure};
use wificalling_location_gateway::service::dispatch::{DispatchError, ServiceDispatch};
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

/// Runtime control for the single OpenWrt daemon. The daemon owns the proxy;
/// the helper owns only the named WLOC nftables table.
struct OpenWrtRuntime;

impl RuntimeControl for OpenWrtRuntime {
    fn start_engine_passthrough(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn engine_healthy(&mut self) -> Result<bool, RuntimeFailure> {
        Ok(shared_gateway_engine_healthy())
    }
    fn arm_watchdog(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn install_exact_redirect(&mut self) -> Result<(), RuntimeFailure> {
        run_redirect_helper(None)
    }
    fn remove_redirect(&mut self) -> Result<(), RuntimeFailure> {
        run_redirect_helper(Some("stop"))
    }
    fn redirect_present(&mut self) -> Result<bool, RuntimeFailure> {
        Ok(std::process::Command::new("nft")
            .args(["list", "table", "inet", "wloc_service"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false))
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

fn run_redirect_helper(action: Option<&str>) -> Result<(), RuntimeFailure> {
    let mut command = std::process::Command::new("/usr/sbin/wloc-redirect-sync.sh");
    if let Some(action) = action {
        command.arg(action);
    }
    command
        .status()
        .map_err(|_| RuntimeFailure)
        .and_then(|status| status.success().then_some(()).ok_or(RuntimeFailure))
}

/// Check the Gateway's already-running sing-box without starting another
/// process. A missing process is a hard health failure: keeping WLOC's
/// redirect installed would send the device into a dead proxy path.
fn shared_gateway_engine_healthy() -> bool {
    std::fs::read_dir("/proc")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            name.to_str()?.parse::<u32>().ok()
        })
        .any(|pid| {
            std::fs::read(format!("/proc/{pid}/cmdline"))
                .map(|cmdline| is_shared_singbox_cmdline(&cmdline))
                .unwrap_or(false)
        })
}

fn is_shared_singbox_cmdline(cmdline: &[u8]) -> bool {
    let mut singbox = false;
    let mut run = false;
    let mut gateway_config = false;
    for argument in cmdline.split(|byte| *byte == 0) {
        if argument == b"run" {
            run = true;
        }
        if argument == b"/var/run/wificalling-gateway/sing-box.json" {
            gateway_config = true;
        }
        let basename = argument
            .rsplit(|byte| *byte == b'/')
            .next()
            .unwrap_or(argument);
        if matches!(basename, b"sing-box" | b"sing-box-lite") {
            singbox = true;
        }
    }
    singbox && run && gateway_config
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
/// The probe wiring (assigned device, probe port, node) comes from the UCI
/// configuration; `WLOC_*` environment variables override it for staging.
fn build_probe(assigned_device: &str) -> Box<dyn ExitProbeRuntime> {
    if std::env::var("WLOC_PROBE").as_deref() == Ok("stub") {
        return Box::new(StubProbe {
            exit_ip: env_or("WLOC_STUB_EXIT_IP", IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))),
            wan_ip: env_or("WLOC_STUB_WAN_IP", IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))),
        });
    }
    // 0.0.0.0 matches no device policy, so the probe falls back to the
    // first non-direct outbound when nothing is configured.
    let device_ip: IpAddr = env_or(
        "WLOC_DEVICE_IP",
        assigned_device
            .parse()
            .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
    );
    Box::new(
        wificalling_location_gateway::exitprobe::singbox::SingBoxProbe::new(
            std::path::PathBuf::from(
                std::env::var("WLOC_SINGBOX_CONFIG")
                    .unwrap_or_else(|_| "/var/run/wificalling-gateway/sing-box.json".into()),
            ),
            device_ip,
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
                city: "Stub City".to_owned(),
                latitude: self.latitude,
                longitude: self.longitude,
                timezone: "UTC".to_owned(),
                expires_at_unix: now_unix() + 3_600,
            },
        )))
    }
}

/// Short-lived proxy handshake health for the admin UI.
#[derive(Default)]
struct ProxyHealth {
    last_ok: Option<u64>,
    last_failure: Option<u64>,
    failures: u64,
}

fn write_proxy_health(path: &Path, health: &ProxyHealth) -> std::io::Result<()> {
    let snapshot = serde_json::json!({
        "last_success": health.last_ok,
        "last_failure": health.last_failure,
        "failures": health.failures,
    });
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string(&snapshot).unwrap_or_default())
}

/// A health file belongs to one daemon lifetime; never inherit a stopped
/// process's failures into a new, healthy listener.
fn reset_proxy_health(path: &Path) -> std::io::Result<()> {
    write_proxy_health(path, &ProxyHealth::default())
}

/// Record a successful or failed proxy connection and rewrite the health
/// file so the certificate-trust state is visible without log parsing.
fn record_proxy_health(health: &std::sync::Mutex<ProxyHealth>, path: &std::path::Path, ok: bool) {
    let mut guard = match health.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let now = now_unix();
    if ok {
        guard.last_ok = Some(now);
    } else {
        guard.last_failure = Some(now);
        guard.failures = guard.failures.saturating_add(1);
    }
    let _ = write_proxy_health(path, &guard);
}

/// A rejected client certificate/SNI proves the boundary held; it is not a
/// failure of the running WLOC service.
fn proxy_error_degrades_health(error: &MitmProxyError) -> bool {
    !matches!(error, MitmProxyError::ClientTls(_))
}

fn spawn_proxy_connection(
    stream: TcpStream,
    proxy: &std::sync::Arc<MitmProxy>,
    patch_state: &std::sync::Arc<std::sync::Mutex<Option<PatchTarget>>>,
    proxy_slots: &std::sync::Arc<tokio::sync::Semaphore>,
    proxy_health: &std::sync::Arc<std::sync::Mutex<ProxyHealth>>,
    health_path: &str,
) {
    let Ok(slot) = proxy_slots.clone().try_acquire_owned() else {
        return;
    };
    let proxy = std::sync::Arc::clone(proxy);
    let patch_state = std::sync::Arc::clone(patch_state);
    let proxy_health = std::sync::Arc::clone(proxy_health);
    let health_path = health_path.to_owned();
    tokio::spawn(async move {
        let result = tokio::time::timeout(
            PROXY_CONNECTION_TIMEOUT,
            proxy.handle_connection(stream, patch_state),
        )
        .await
        .map_err(|_| MitmProxyError::Upstream("client timeout".into()))
        .and_then(|result| result);
        match result {
            Ok(()) => record_proxy_health(&proxy_health, std::path::Path::new(&health_path), true),
            Err(error) => {
                eprintln!("wloc proxy: connection error: {error}");
                if proxy_error_degrades_health(&error) {
                    record_proxy_health(&proxy_health, std::path::Path::new(&health_path), false);
                }
            }
        }
        drop(slot);
    });
}

/// Bind one transparent TCP listener. Linux needs IP_TRANSPARENT for TPROXY;
/// dev/test platforms use the same socket shape without that Linux option.
fn bind_tproxy_listener(
    domain: socket2::Domain,
    address: std::net::SocketAddr,
    only_v6: bool,
) -> std::io::Result<tokio::net::TcpListener> {
    let socket = socket2::Socket::new(domain, socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
    socket.set_reuse_address(true)?;
    if domain == socket2::Domain::IPV6 {
        socket.set_only_v6(only_v6)?;
    }
    #[cfg(target_os = "linux")]
    if domain == socket2::Domain::IPV6 {
        socket.set_ip_transparent_v6(true)?;
    } else {
        socket.set_ip_transparent_v4(true)?;
    }
    socket.bind(&address.into())?;
    socket.listen(1024)?;
    let std_listener: std::net::TcpListener = socket.into();
    std_listener.set_nonblocking(true)?;
    tokio::net::TcpListener::from_std(std_listener)
}

fn bind_tproxy_listener_v4(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    bind_tproxy_listener(socket2::Domain::IPV4, ([0, 0, 0, 0], port).into(), true)
}

const MAX_PROXY_CONNECTIONS: usize = 8;
const PROXY_CONNECTION_TIMEOUT: Duration = Duration::from_secs(60);

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let socket_path = std::env::var("WLOC_SOCKET")
        .unwrap_or_else(|_| "/var/run/wloc-service/control.sock".into());

    // Persisted configuration from /etc/config/wloc-service (UCI). A missing
    // file falls back to the defaults so the daemon still runs unconfigured.
    let uci_path = std::env::var("WLOC_UCI_CONFIG")
        .unwrap_or_else(|_| wificalling_location_gateway::config::uci::DEFAULT_UCI_PATH.into());
    let uci = match WlocUciConfig::load(Path::new(&uci_path)) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("wloc-service: {error}; using defaults");
            WlocUciConfig::default()
        }
    };
    // The device whose node binding the location follows. It is normally
    // chosen from LuCI; when unset, fall back to the first device policy of
    // the Gateway config so a fresh install follows something on any subnet
    // instead of a fixed example address.
    let assigned_device = if uci.assigned_device.trim().is_empty() {
        gateway_first_device_ip().unwrap_or_default()
    } else {
        uci.assigned_device.clone()
    };
    eprintln!(
        "wloc-service: uci enabled={} geo_source={:?} node={} device={}",
        uci.enabled,
        uci.location_mode,
        uci.node_ref,
        if assigned_device.is_empty() {
            "(none)".to_owned()
        } else {
            assigned_device.clone()
        }
    );

    // Default to the real Geo HTTP provider; geo_provider=stub (UCI or
    // WLOC_GEO_PROVIDER) forces the deterministic stub for offline work.
    let geo_provider: String =
        std::env::var("WLOC_GEO_PROVIDER").unwrap_or_else(|_| uci.geo_provider.clone());
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
        OpenWrtRuntime,
        build_probe(&assigned_device),
        geo,
        WlocServiceConfig {
            node_ref: NodeRef::new(&uci.node_ref)
                .unwrap_or_else(|_| NodeRef::new("default").expect("static node ref is valid")),
            providers: vec![ProviderRef::new("http").expect("static provider ref is valid")],
            probe_limits: ProbeLimits {
                max_observation_age: Duration::from_secs(uci.probe_interval_secs),
            },
            scope_valid: true,
            ipv6_ready: false,
            assigned_device_configured: !assigned_device.is_empty(),
            assigned_device: if assigned_device.is_empty() {
                None
            } else {
                Some(assigned_device)
            },
            // Background manual place-info lookups use the public Nominatim
            // TLS endpoint; a strict connect/read timeout keeps them off the
            // control path.
            reverse_geo_lookup: Some(("nominatim.openstreetmap.org".to_owned(), 443)),
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
    let ca_info_path = Path::new(&ca_path).with_extension("info.json");
    let ca_issued_at_unix;
    let mitm_ca = if Path::new(&ca_key_path).exists() && Path::new(&ca_path).exists() {
        let key_der = std::fs::read(&ca_key_path)?;
        let cert_der = pem_decode(&std::fs::read(&ca_path)?)?;
        ca_issued_at_unix = read_ca_info(&ca_info_path)
            .ok()
            .and_then(|info| info.get("issued_at").and_then(|v| v.as_i64()))
            .or_else(|| {
                // Legacy CA without an info file: approximate from the file
                // modification time.
                std::fs::metadata(&ca_path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
            });
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
        ca_issued_at_unix = Some(now_unix() as i64);
        eprintln!("MITM root CA generated; export at {ca_path} (install on the test device)");
        ca
    };
    eprintln!("MITM root CA ready (private key: {ca_key_path})");
    // Expose the CA basics (fingerprint, issue/expiry) for the admin UI.
    let ca_info = serde_json::json!({
        "fingerprint": mitm_ca.fingerprint_sha256(),
        "issued_at": ca_issued_at_unix.unwrap_or(now_unix() as i64),
        "expires_at": mitm_ca.not_after_unix(),
    });
    if let Some(parent) = ca_info_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&ca_info_path, serde_json::to_string_pretty(&ca_info)?)?;

    // Ship the iOS CA profile right after the root CA is ready, so the LuCI
    // profile link works without an explicit "Regenerate profile" click
    // (the export script writes /www/wloc-ca.mobileconfig for uhttpd). The
    // script is part of the package and may be absent in dev builds - both
    // failure modes are non-fatal.
    if let Ok(output) = std::process::Command::new("/usr/sbin/export-mobileconfig.sh").output() {
        if !output.status.success() {
            eprintln!(
                "warning: export-mobileconfig.sh failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
    }

    let mut upstream_roots = rustls::RootCertStore::empty();
    upstream_roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let events_file = std::env::var("WLOC_EVENTS_FILE")
        .unwrap_or_else(|_| "/var/run/wloc-service/events.jsonl".into());
    let upstream_file = std::env::var("WLOC_UPSTREAM_IP_FILE")
        .unwrap_or_else(|_| "/var/run/wloc-service/upstream-map".into());
    let proxy = MitmProxy::new(&mitm_ca, upstream_roots)?
        .with_events_file(std::path::PathBuf::from(events_file))
        .with_upstream_override_file(std::path::PathBuf::from(upstream_file));
    // Production traffic uses its original public TPROXY destination. When
    // local DNS maps an approved name to the router, the proxy resolves that
    // name through its own host-specific public mapping.
    let proxy = std::sync::Arc::new(proxy);
    let proxy_port: u16 = env_or("WLOC_PROXY_PORT", 8443_u16);

    let patch_state = std::sync::Arc::new(std::sync::Mutex::new(None::<PatchTarget>));
    let mut service = service
        .with_patch_sink(std::sync::Arc::clone(&patch_state))
        .with_state_files(
            std::path::PathBuf::from(
                std::env::var("WLOC_STATUS_FILE")
                    .unwrap_or_else(|_| "/var/run/wloc-service/status.json".into()),
            ),
            std::path::PathBuf::from(
                std::env::var("WLOC_EVENTS_FILE")
                    .unwrap_or_else(|_| "/var/run/wloc-service/events.jsonl".into()),
            ),
        );

    // Bind the proxy before enabling the redirect. This makes the runtime
    // health check meaningful: no packet can be sent to :8443 before a
    // listener exists.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let proxy_listener = runtime.block_on(async { bind_tproxy_listener_v4(proxy_port) })?;

    // Apply the persisted configuration to the control plane before serving:
    // manual location preset first (so a manual target is already fresh), then
    // the desired enabled state. Failures are logged, not fatal: the daemon
    // still serves status and can be steered through the control API.
    if uci.location_mode == LocationMode::Manual {
        if let (Some(latitude), Some(longitude)) = (uci.manual_latitude, uci.manual_longitude) {
            let params = RequestParams {
                query: None,
                latitude: Some(latitude),
                longitude: Some(longitude),
            };
            if let Err(error) = service.set_manual_location(&params) {
                eprintln!("wloc-service: applying manual location failed: {error:?}");
            }
        } else {
            eprintln!(
                "wloc-service: geo_source=manual but manual_lat/manual_lon unset; keeping auto"
            );
        }
    }
    if uci.enabled {
        // procd starts the Gateway before WLOC, but the Gateway's listener is
        // asynchronous. Retry only the recoverable missing-target case so a
        // reboot does not leave WLOC permanently disabled just because the
        // existing sing-box was still binding its probe inbounds.
        let mut enabled = false;
        for attempt in 0..10 {
            match service.enable() {
                Ok(()) => {
                    enabled = true;
                    break;
                }
                Err(DispatchError::Unavailable) if attempt < 9 => {
                    std::thread::sleep(Duration::from_secs(1));
                }
                Err(error) => {
                    eprintln!("wloc-service: enable failed: {error:?}");
                    break;
                }
            }
        }
        if !enabled {
            eprintln!("wloc-service: automatic location target is unavailable");
        }
    }

    if let Some(parent) = Path::new(&socket_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&socket_path);

    let listener = runtime.block_on(async { tokio::net::UnixListener::bind(&socket_path) })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    }

    // Proxy TLS health: whether the test devices trust the current root CA.
    // A TLS handshake failure (untrusted CA, wrong SNI, ...) is recorded so
    // the admin UI can show "certificate not trusted" instead of a mystery.
    let proxy_health = std::sync::Arc::new(std::sync::Mutex::new(ProxyHealth::default()));
    let health_path = std::env::var("WLOC_HEALTH_FILE")
        .unwrap_or_else(|_| "/var/run/wloc-service/proxy-health.json".into());
    reset_proxy_health(Path::new(&health_path))?;
    let proxy_slots = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_PROXY_CONNECTIONS));

    // TPROXY listener: IP_TRANSPARENT lets the kernel deliver connections
    // whose original destination is a remote host (the Apple WLOC IP), so
    // iOS sees a perfectly normal connection to the Apple server - unlike
    // REDIRECT, which rewrites the destination to this router and newer iOS
    // versions answer with RST.
    runtime.spawn(async move {
        loop {
            tokio::select! {
                accepted = proxy_listener.accept() => match accepted {
                    Ok((stream, _)) => spawn_proxy_connection(
                        stream, &proxy, &patch_state, &proxy_slots, &proxy_health, &health_path,
                    ),
                    Err(error) => eprintln!("wloc proxy: IPv4 accept error: {error}"),
                },
            }
        }
    });
    eprintln!("wloc-service MITM proxy listening on 0.0.0.0:{proxy_port}");

    eprintln!("wloc-service listening on {socket_path}");
    let server = ControlServer::new(service);
    // Housekeeping runs every 10s to publish local status and completed
    // background results. In auto mode it rechecks exit evidence when the
    // configured observation age is reached; manual mode never probes IP.
    runtime.block_on(server.serve(listener, std::time::Duration::from_secs(10)));
    Ok(())
}

/// The first source IP of the Gateway device policy - the natural follow
/// target when wloc-service has no assigned device configured.
fn gateway_first_device_ip() -> Option<String> {
    let output = std::process::Command::new("uci")
        .args(["-q", "get", "wificalling-gateway.@device[0].source_ip"])
        .output()
        .ok()?;
    let ip = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if ip.is_empty() {
        None
    } else {
        Some(ip)
    }
}

/// Read the persisted CA info JSON (fingerprint/issued_at/expires_at).
fn read_ca_info(path: &Path) -> Result<serde_json::Value, Box<dyn Error + Send + Sync>> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
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

#[cfg(test)]
mod tests {
    use super::*;
    use wificalling_location_gateway::mitm::CaBundle;

    #[test]
    fn recognizes_only_a_running_shared_singbox_command() {
        assert!(is_shared_singbox_cmdline(
            b"/tmp/sing-box-lite\0run\0-c\0/var/run/wificalling-gateway/sing-box.json\0"
        ));
        assert!(is_shared_singbox_cmdline(
            b"/usr/bin/sing-box\0run\0-c\0/var/run/wificalling-gateway/sing-box.json\0"
        ));
        assert!(!is_shared_singbox_cmdline(
            b"/usr/bin/sing-box\0run\0-c\0/etc/passwall/sing-box.json\0"
        ));
        assert!(!is_shared_singbox_cmdline(b"/usr/sbin/wloc-service\0"));
        assert!(!is_shared_singbox_cmdline(b"/tmp/sing-box-lite\0check\0"));
    }

    #[test]
    fn pem_certificate_round_trip_preserves_der() {
        let ca = CaBundle::generate().expect("test CA must generate");
        let der = ca.root_cert_der();
        let pem = pem_encode(&der);
        assert_eq!(pem_decode(pem.as_bytes()).unwrap(), der.as_ref());
    }

    #[test]
    fn rejected_client_tls_does_not_degrade_proxy_health() {
        assert!(!proxy_error_degrades_health(&MitmProxyError::ClientTls(
            "unapproved SNI".into()
        )));
        assert!(proxy_error_degrades_health(&MitmProxyError::Upstream(
            "connection refused".into()
        )));
    }

    #[test]
    fn startup_health_snapshot_drops_prior_process_failures() {
        let path = std::env::temp_dir().join(format!(
            "wloc-proxy-health-{}-{}.json",
            std::process::id(),
            now_unix()
        ));
        std::fs::write(
            &path,
            r#"{"last_success":1,"last_failure":2,"failures":43}"#,
        )
        .unwrap();

        reset_proxy_health(&path).unwrap();

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&path).unwrap())
                .unwrap(),
            serde_json::json!({"last_success": null, "last_failure": null, "failures": 0})
        );
        let _ = std::fs::remove_file(path);
    }
}
