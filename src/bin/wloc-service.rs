//! Run the WLOC service control daemon over a root-owned Unix socket.
//!
//! The daemon serves the frozen control API on a local Unix socket. Its
//! OpenWrt runtime adapter delegates only the component-owned WLOC redirect;
//! process ownership and the optional sing-box provider remain with the
//! standalone supervisor.
//!
//! Socket path: `WLOC_SOCKET` (default `/var/run/wloc-service/control.sock`).

use std::collections::HashMap;
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wificalling_location_gateway::app::{WlocService, WlocServiceConfig};
use wificalling_location_gateway::config::{
    DeviceProfile, LocationMode, ProfileModel, RuntimeProfile, WlocUciConfig,
};
use wificalling_location_gateway::exitprobe::runtime::{ExitProbeRuntime, ProbeFailure};
use wificalling_location_gateway::exitprobe::{NodeRef, ProbeLimits};
use wificalling_location_gateway::georesolver::http::GeoHttpClient;
use wificalling_location_gateway::georesolver::runtime::{GeoProviderRuntime, ProviderFailure};
use wificalling_location_gateway::georesolver::ProviderRef;
use wificalling_location_gateway::mitm::proxy::MitmProxy;
use wificalling_location_gateway::mitm::CaBundle;
use wificalling_location_gateway::service::api::RequestParams;
use wificalling_location_gateway::service::control::{RuntimeControl, RuntimeFailure};
use wificalling_location_gateway::service::dispatch::{
    DispatchError, ProfileDispatch, ServiceDispatch, UciProfileStore,
};
use wificalling_location_gateway::service::profile_dispatch::ProfilePatchRouter;
use wificalling_location_gateway::service::profile_runtime::{
    ProfileRuntimeControl, ProfileRuntimeError, ProfileRuntimeManager,
};
use wificalling_location_gateway::service::server::{ControlServer, NoProfileDispatch};
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

fn profile_marker_path(name: &str, default: &str) -> std::path::PathBuf {
    std::env::var(name)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(default))
}

fn write_profile_marker(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, b"ready\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

struct ServicePidMarker {
    path: std::path::PathBuf,
    pid: u32,
}

impl Drop for ServicePidMarker {
    fn drop(&mut self) {
        let current = std::fs::read_to_string(&self.path).ok();
        if current.as_deref().map(str::trim) == Some(&self.pid.to_string()) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn write_service_pid_marker(path: &Path) -> std::io::Result<ServicePidMarker> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let pid = std::process::id();
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, format!("{pid}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&temporary, path)?;
    Ok(ServicePidMarker {
        path: path.to_owned(),
        pid,
    })
}

fn wait_for_profile_marker(path: &std::path::Path, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while !path.exists() {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    true
}

fn disabled_runtime_profile() -> RuntimeProfile {
    RuntimeProfile {
        id: "invalid".to_owned(),
        enabled: false,
        runtime_supported: false,
        assigned_device: None,
        node_ref: "default".to_owned(),
        location_mode: LocationMode::Auto,
        manual_latitude: None,
        manual_longitude: None,
    }
}

fn runtime_profile_from_uci(uci: &WlocUciConfig) -> RuntimeProfile {
    match uci
        .profile_model()
        .and_then(|model| model.single_runtime_profile())
    {
        Ok(mut profile) => {
            if !profile.runtime_supported {
                eprintln!(
                    "wloc-service: profile {} has no IP runtime binding; staying disabled",
                    profile.id
                );
                profile.enabled = false;
            }
            profile
        }
        Err(error) => {
            eprintln!("wloc-service: profile is not runnable yet: {error}; staying disabled");
            disabled_runtime_profile()
        }
    }
}

fn runtime_scope_valid(config_valid: bool, profile: &RuntimeProfile) -> bool {
    config_valid && profile.runtime_supported
}

/// Runtime control for the daemon's OpenWrt boundary.
///
/// The outer unified supervisor owns process start/stop and watchdogs. This
/// daemon delegates the component-owned redirect and queries its presence;
/// self-stop/drain operations are no-ops because the control server must not
/// terminate itself.
struct OpenWrtRuntime {
    redirect_helper: std::path::PathBuf,
    profile_redirect_helper: std::path::PathBuf,
    nft_binary: std::path::PathBuf,
    control_socket: std::path::PathBuf,
    proxy_port: u16,
    defer_first_redirect: bool,
}

impl OpenWrtRuntime {
    fn from_env() -> Self {
        let mut runtime = Self::new(
            std::env::var("WLOC_REDIRECT_HELPER")
                .unwrap_or_else(|_| "/usr/sbin/wloc-redirect-sync.sh".to_owned()),
            std::env::var("WLOC_NFT_BINARY").unwrap_or_else(|_| "nft".to_owned()),
        );
        runtime.profile_redirect_helper = std::env::var("WLOC_PROFILE_REDIRECT_HELPER")
            .unwrap_or_else(|_| "/usr/sbin/wloc-profile-redirect.sh".to_owned())
            .into();
        runtime.control_socket =
            profile_marker_path("WLOC_SOCKET", "/var/run/wloc-service/control.sock");
        runtime.proxy_port = env_or("WLOC_PROXY_PORT", 8443_u16);
        runtime.defer_first_redirect = std::env::var("WLOC_DEFER_REDIRECT").as_deref() == Ok("1");
        runtime
    }

    fn new(
        redirect_helper: impl Into<std::path::PathBuf>,
        nft_binary: impl Into<std::path::PathBuf>,
    ) -> Self {
        let redirect_helper = redirect_helper.into();
        Self {
            profile_redirect_helper: redirect_helper.clone(),
            redirect_helper,
            nft_binary: nft_binary.into(),
            control_socket: "/var/run/wloc-service/control.sock".into(),
            proxy_port: 8443,
            defer_first_redirect: false,
        }
    }

    #[cfg(test)]
    fn with_health_paths(
        mut self,
        control_socket: impl Into<std::path::PathBuf>,
        proxy_port: u16,
    ) -> Self {
        self.control_socket = control_socket.into();
        self.proxy_port = proxy_port;
        self
    }

    #[cfg(test)]
    fn defer_first_redirect(&mut self) {
        self.defer_first_redirect = true;
    }

    fn run_redirect(&self, action: &str) -> Result<(), RuntimeFailure> {
        std::process::Command::new(&self.redirect_helper)
            .arg(action)
            .status()
            .map_err(|_| RuntimeFailure)
            .and_then(|status| status.success().then_some(()).ok_or(RuntimeFailure))
    }

    fn run_profile_redirect(
        &self,
        action: &str,
        profile_id: &str,
        assigned_device: Option<&str>,
    ) -> Result<(), ProfileRuntimeError> {
        let mut command = std::process::Command::new(&self.profile_redirect_helper);
        command.arg(action).arg(profile_id);
        if let Some(device) = assigned_device {
            command.arg(device);
        }
        command
            .status()
            .map_err(|_| ProfileRuntimeError::RedirectInstall)
            .and_then(|status| {
                status
                    .success()
                    .then_some(())
                    .ok_or(ProfileRuntimeError::RedirectInstall)
            })
    }

    fn service_evidence_present(&self) -> bool {
        #[cfg(unix)]
        let socket_ready = std::fs::metadata(&self.control_socket)
            .map(|metadata| {
                use std::os::unix::fs::FileTypeExt;
                metadata.file_type().is_socket()
            })
            .unwrap_or(false);
        #[cfg(not(unix))]
        let socket_ready = false;
        if !socket_ready {
            return false;
        }

        let proxy_address = SocketAddr::from(([127, 0, 0, 1], self.proxy_port));
        TcpStream::connect_timeout(&proxy_address, Duration::from_millis(250)).is_ok()
    }
}

impl RuntimeControl for OpenWrtRuntime {
    fn start_engine_passthrough(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn engine_healthy(&mut self) -> Result<bool, RuntimeFailure> {
        Ok(self.service_evidence_present())
    }
    fn arm_watchdog(&mut self) -> Result<(), RuntimeFailure> {
        Ok(())
    }
    fn install_exact_redirect(&mut self) -> Result<(), RuntimeFailure> {
        if self.defer_first_redirect {
            self.defer_first_redirect = false;
            return Ok(());
        }
        self.run_redirect("start")
    }
    fn remove_redirect(&mut self) -> Result<(), RuntimeFailure> {
        self.run_redirect("stop")
    }
    fn redirect_present(&mut self) -> Result<bool, RuntimeFailure> {
        std::process::Command::new(&self.nft_binary)
            .args(["list", "table", "inet", "wloc_service"])
            .status()
            .map(|status| status.success())
            .map_err(|_| RuntimeFailure)
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

impl ProfileRuntimeControl for OpenWrtRuntime {
    fn ensure_shared_engine(&mut self) -> Result<(), ProfileRuntimeError> {
        // The standalone supervisor owns the single WLOC/provider process set.
        Ok(())
    }

    fn shared_engine_healthy(&mut self) -> Result<bool, ProfileRuntimeError> {
        Ok(self.service_evidence_present())
    }

    fn install_profile_redirect(
        &mut self,
        profile: &DeviceProfile,
    ) -> Result<(), ProfileRuntimeError> {
        let address = profile
            .assigned_device
            .as_deref()
            .filter(|address| address.parse::<std::net::Ipv4Addr>().is_ok())
            .ok_or(ProfileRuntimeError::UnsupportedDevice)?;
        self.run_profile_redirect("start", &profile.id, Some(address))
    }

    fn remove_profile_redirect(&mut self, profile_id: &str) -> Result<(), ProfileRuntimeError> {
        self.run_profile_redirect("stop", profile_id, None)
    }

    fn profile_redirect_present(&mut self, profile_id: &str) -> Result<bool, ProfileRuntimeError> {
        let status = std::process::Command::new(&self.profile_redirect_helper)
            .args(["status", profile_id])
            .status()
            .map_err(|_| ProfileRuntimeError::CleanupUnsafe)?;
        Ok(status.success())
    }
}

/// Per-profile WlocService state does not own the shared provider process.
/// The standalone procd supervisor and `ProfileRuntimeManager` own that process
/// and each profile's redirect; this adapter keeps the profile's probe/Geo
/// state machine independent without spawning another engine.
#[derive(Default)]
struct ProfileServiceRuntime {
    logical_redirect_present: bool,
}

impl RuntimeControl for ProfileServiceRuntime {
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
        // The profile redirect is installed by ProfileRuntimeManager. This
        // handler still needs a local lifecycle bit so WlocService's safety
        // state machine can verify its own enable/disable transaction without
        // trying to inspect or mutate another profile's nft table.
        self.logical_redirect_present = true;
        Ok(())
    }

    fn remove_redirect(&mut self) -> Result<(), RuntimeFailure> {
        self.logical_redirect_present = false;
        Ok(())
    }

    fn redirect_present(&mut self) -> Result<bool, RuntimeFailure> {
        Ok(self.logical_redirect_present)
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

type BoxedProfileService = Box<dyn ServiceDispatch>;

/// One control-plane facade over independent per-profile probe/Geo handlers
/// and the shared profile redirect manager. V1 requests retain their legacy
/// meaning and operate on the deterministic default profile; periodic work
/// refreshes every enabled profile.
struct ProfileServiceGroup {
    default_profile_id: String,
    handlers: HashMap<String, BoxedProfileService>,
    runtime: ProfileRuntimeManager<OpenWrtRuntime>,
    router: std::sync::Arc<ProfilePatchRouter>,
}

impl ProfileServiceGroup {
    fn new(
        model: ProfileModel,
        router: std::sync::Arc<ProfilePatchRouter>,
        handlers: HashMap<String, BoxedProfileService>,
        runtime: OpenWrtRuntime,
    ) -> Self {
        let ready = profile_marker_path(
            "WLOC_PROFILE_READY_FILE",
            "/var/run/wloc-service/profiles/.ready",
        );
        let activate = profile_marker_path(
            "WLOC_PROFILE_ACTIVATE_FILE",
            "/var/run/wloc-service/profiles/.activate",
        );
        let _ = std::fs::remove_file(ready);
        let _ = std::fs::remove_file(activate);
        let default_profile_id = model
            .profiles()
            .first()
            .map(|profile| profile.id.clone())
            .unwrap_or_else(|| "default".to_owned());
        Self {
            default_profile_id,
            handlers,
            runtime: ProfileRuntimeManager::new(model, runtime),
            router,
        }
    }

    fn activate_enabled_profiles(&mut self) {
        let ids: Vec<String> = self
            .runtime
            .model()
            .profiles()
            .iter()
            .filter(|profile| profile.enabled)
            .map(|profile| profile.id.clone())
            .collect();
        for profile_id in ids {
            if self.enable_profile(&profile_id).is_err() {
                let _ = self.router.set_enabled(&profile_id, false);
            }
        }
        let ready = profile_marker_path(
            "WLOC_PROFILE_READY_FILE",
            "/var/run/wloc-service/profiles/.ready",
        );
        if let Err(error) = write_profile_marker(&ready) {
            eprintln!("wloc-service: profile readiness marker failed: {error}");
        }
    }

    fn enable_profile(&mut self, profile_id: &str) -> Result<(), DispatchError> {
        self.runtime
            .enable(profile_id)
            .map_err(map_profile_runtime_error)?;
        let handler = self
            .handlers
            .get_mut(profile_id)
            .ok_or(DispatchError::InvalidConfig)?;
        if let Err(error) = handler.enable() {
            let _ = self.runtime.disable(profile_id);
            let _ = self.router.set_enabled(profile_id, false);
            return Err(error);
        }
        self.router
            .set_enabled(profile_id, true)
            .map_err(|_| DispatchError::RuntimeFailure)
    }

    fn disable_profile(&mut self, profile_id: &str) -> Result<(), DispatchError> {
        let handler = self
            .handlers
            .get_mut(profile_id)
            .ok_or(DispatchError::InvalidConfig)?;
        let handler_result = handler.disable();
        let runtime_result = self
            .runtime
            .disable(profile_id)
            .map_err(map_profile_runtime_error);
        let _ = self.router.set_enabled(profile_id, false);
        handler_result.and(runtime_result)
    }

    fn default_handler(&mut self) -> Result<&mut BoxedProfileService, DispatchError> {
        self.handlers
            .get_mut(&self.default_profile_id)
            .ok_or(DispatchError::InvalidConfig)
    }
}

impl ServiceDispatch for ProfileServiceGroup {
    fn activate_profiles(&mut self) {
        self.activate_enabled_profiles();
    }

    fn status(&mut self) -> Result<serde_json::Value, DispatchError> {
        self.default_handler()?.status()
    }

    fn enable(&mut self) -> Result<(), DispatchError> {
        self.enable_profile(&self.default_profile_id.clone())
    }

    fn disable(&mut self) -> Result<(), DispatchError> {
        self.disable_profile(&self.default_profile_id.clone())
    }

    fn reload(&mut self) -> Result<(), DispatchError> {
        self.default_handler()?.reload()
    }

    fn set_manual_location(&mut self, params: &RequestParams) -> Result<(), DispatchError> {
        self.default_handler()?.set_manual_location(params)
    }

    fn clear_manual_location(&mut self) -> Result<(), DispatchError> {
        self.default_handler()?.clear_manual_location()
    }

    fn search_location(&mut self, query: &str) -> Result<serde_json::Value, DispatchError> {
        self.default_handler()?.search_location(query)
    }

    fn refresh_periodic(&mut self) {
        for handler in self.handlers.values_mut() {
            handler.refresh_periodic();
        }
    }

    fn refresh_evidence(&mut self) -> Result<(), DispatchError> {
        self.default_handler()?.refresh_evidence()
    }
}

fn map_profile_runtime_error(error: ProfileRuntimeError) -> DispatchError {
    match error {
        ProfileRuntimeError::EngineStart => DispatchError::RuntimeFailure,
        ProfileRuntimeError::EngineUnhealthy => DispatchError::EngineUnhealthy,
        ProfileRuntimeError::RedirectInstall => DispatchError::RuntimeFailure,
        ProfileRuntimeError::RedirectStillPresent => DispatchError::RedirectPresent,
        ProfileRuntimeError::CleanupUnsafe => DispatchError::CleanupUnsafe,
        ProfileRuntimeError::UnsupportedDevice | ProfileRuntimeError::ProfileDisabled => {
            DispatchError::InvalidConfig
        }
        ProfileRuntimeError::UnknownProfile => DispatchError::InvalidConfig,
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
/// The probe wiring (assigned device, probe port, node) comes from the UCI
/// configuration; `WLOC_*` environment variables override it for staging.
fn build_probe(assigned_device: &str, probe_port: u16) -> Box<dyn ExitProbeRuntime> {
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
    let probe_port: u16 = env_or("WLOC_PROBE_PORT", probe_port);
    let probe = wificalling_location_gateway::exitprobe::singbox::SingBoxProbe::new(
        std::path::PathBuf::from(
            std::env::var("WLOC_SINGBOX_CONFIG")
                .unwrap_or_else(|_| "/var/run/wloc-service/sing-box.json".into()),
        ),
        device_ip,
        probe_port,
        std::path::PathBuf::from("/tmp/wloc-probe"),
    )
    .with_singbox_bin(
        std::env::var("WLOC_SINGBOX_BIN").unwrap_or_else(|_| "/usr/bin/sing-box".to_owned()),
    );
    if assigned_device.trim().is_empty() {
        Box::new(probe)
    } else {
        Box::new(probe.with_required_device_binding())
    }
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

fn build_geo_provider(geo_provider: &str) -> Box<dyn GeoProviderRuntime> {
    if geo_provider == "stub" {
        Box::new(StubGeo {
            country_code: env_or("WLOC_STUB_COUNTRY", "US".to_owned()),
            latitude: env_or("WLOC_STUB_LAT", 37.77_f64),
            longitude: env_or("WLOC_STUB_LON", -122.41_f64),
        })
    } else {
        Box::new(GeoHttpClient::ip_api_default())
    }
}

fn build_profile_handler(
    profile: &DeviceProfile,
    uci: &WlocUciConfig,
    config_valid: bool,
    geo_provider: &str,
    router: &std::sync::Arc<ProfilePatchRouter>,
) -> Result<BoxedProfileService, String> {
    let assigned_device = profile.assigned_device.clone().unwrap_or_default();
    let assigned_ip = assigned_device
        .parse::<IpAddr>()
        .map_err(|_| "profile device is not an IP address".to_owned())?;
    let sink = router
        .profile_sink(&profile.id)
        .map_err(|_| "profile patch sink is unavailable".to_owned())?;
    let state_dir = std::env::var("WLOC_PROFILE_STATE_DIR")
        .unwrap_or_else(|_| "/var/run/wloc-service/profiles".to_owned());
    let profile_dir = Path::new(&state_dir).join(&profile.id);
    let mut service = WlocService::new(
        ProfileServiceRuntime::default(),
        build_probe(&assigned_device, uci.probe_port),
        build_geo_provider(geo_provider),
        WlocServiceConfig {
            node_ref: NodeRef::new(&profile.node_ref)
                .map_err(|_| "invalid profile node reference".to_owned())?,
            providers: vec![ProviderRef::new("http").expect("static provider ref is valid")],
            probe_limits: ProbeLimits {
                max_observation_age: Duration::from_secs(uci.probe_interval_secs),
            },
            scope_valid: config_valid && matches!(assigned_ip, IpAddr::V4(_)),
            ipv6_ready: true,
            assigned_device_configured: true,
            assigned_device: Some(assigned_device),
            reverse_geo_lookup: Some(("nominatim.openstreetmap.org".to_owned(), 443)),
        },
    )
    .with_patch_sink(sink)
    .with_state_files(
        profile_dir.join("status.json"),
        profile_dir.join("events.jsonl"),
    );

    if profile.location_mode == LocationMode::Manual {
        let (Some(latitude), Some(longitude)) = (profile.manual_latitude, profile.manual_longitude)
        else {
            return Err("manual profile is missing coordinates".to_owned());
        };
        service
            .set_manual_location(&RequestParams {
                query: None,
                latitude: Some(latitude),
                longitude: Some(longitude),
            })
            .map_err(|_| "manual profile coordinates were rejected".to_owned())?;
    }
    Ok(Box::new(service))
}

/// Short-lived proxy handshake health for the admin UI.
#[derive(Default)]
struct ProxyHealth {
    last_ok: Option<u64>,
    last_failure: Option<u64>,
    failures: u64,
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
    let snapshot = serde_json::json!({
        "last_success": guard.last_ok,
        "last_failure": guard.last_failure,
        "failures": guard.failures,
    });
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, serde_json::to_string(&snapshot).unwrap_or_default());
}

/// Bind the MITM proxy listener with the TPROXY transparent flag (Linux).
/// On non-Linux platforms (dev/test builds) it falls back to a plain bind.
fn bind_tproxy_listener(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    let domain = socket2::Domain::IPV4;
    let socket = socket2::Socket::new(domain, socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
    socket.set_reuse_address(true)?;
    #[cfg(target_os = "linux")]
    socket.set_ip_transparent(true)?;
    let addr: std::net::SocketAddr = ([0, 0, 0, 0], port).into();
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    let std_listener: std::net::TcpListener = socket.into();
    tokio::net::TcpListener::from_std(std_listener)
}

fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let socket_path = std::env::var("WLOC_SOCKET")
        .unwrap_or_else(|_| "/var/run/wloc-service/control.sock".into());

    // Persisted configuration from /etc/config/wloc-service (UCI). A missing
    // file preserves the v1 unconfigured-default behavior; an existing but
    // invalid file is fail-closed and cannot later be enabled over the socket.
    let uci_path = std::env::var("WLOC_UCI_CONFIG")
        .unwrap_or_else(|_| wificalling_location_gateway::config::uci::DEFAULT_UCI_PATH.into());
    let (uci, config_valid) = match WlocUciConfig::load(Path::new(&uci_path)) {
        Ok(config) => (config, true),
        Err(error) if !Path::new(&uci_path).exists() => {
            eprintln!("wloc-service: {error}; using unconfigured defaults");
            (WlocUciConfig::default(), true)
        }
        Err(error) => {
            eprintln!("wloc-service: {error}; using disabled defaults");
            (
                WlocUciConfig {
                    enabled: false,
                    ..WlocUciConfig::default()
                },
                false,
            )
        }
    };
    let runtime_profile = runtime_profile_from_uci(&uci);
    // The device whose node binding the location follows. It is normally
    // chosen from the WLOC device profile. An unset address is intentionally
    // not guessed from another application.
    let assigned_device = if !runtime_profile.runtime_supported {
        String::new()
    } else {
        runtime_profile.assigned_device.clone().unwrap_or_default()
    };
    eprintln!(
        "wloc-service: profile={} enabled={} geo_source={:?} node={} device_configured={}",
        runtime_profile.id,
        runtime_profile.enabled,
        runtime_profile.location_mode,
        runtime_profile.node_ref,
        !assigned_device.is_empty()
    );

    let profile_model = uci.profile_model().ok();
    let profile_router = match profile_model.as_ref() {
        Some(model) if model.profiles().len() > 1 => Some(std::sync::Arc::new(
            ProfilePatchRouter::new(model)
                .map_err(|_| "multiple profiles require IPv4 device bindings")?,
        )),
        None => None,
        Some(_) => None,
    };
    let patch_state = profile_router
        .as_ref()
        .and_then(|router| router.profile_sink(&runtime_profile.id).ok())
        .unwrap_or_else(|| std::sync::Arc::new(std::sync::Mutex::new(None::<PatchTarget>)));

    // Default to the real Geo HTTP provider; geo_provider=stub (UCI or
    // WLOC_GEO_PROVIDER) forces the deterministic stub for offline work.
    let geo_provider: String =
        std::env::var("WLOC_GEO_PROVIDER").unwrap_or_else(|_| uci.geo_provider.clone());
    let mut service: Box<dyn ServiceDispatch> = if let (Some(model), true) = (
        profile_model.clone(),
        profile_model
            .as_ref()
            .is_some_and(|m| m.profiles().len() > 1),
    ) {
        let router = profile_router
            .as_ref()
            .ok_or("profile router is unavailable")?;
        let mut handlers = HashMap::with_capacity(model.profiles().len());
        for profile in model.profiles() {
            let handler =
                build_profile_handler(profile, &uci, config_valid, &geo_provider, router)?;
            handlers.insert(profile.id.clone(), handler);
        }
        let group = ProfileServiceGroup::new(
            model,
            std::sync::Arc::clone(router),
            handlers,
            OpenWrtRuntime::from_env(),
        );
        Box::new(group)
    } else {
        let service = WlocService::new(
            OpenWrtRuntime::from_env(),
            build_probe(&assigned_device, uci.probe_port),
            build_geo_provider(&geo_provider),
            WlocServiceConfig {
                node_ref: NodeRef::new(&runtime_profile.node_ref)
                    .unwrap_or_else(|_| NodeRef::new("default").expect("static node ref is valid")),
                providers: vec![ProviderRef::new("http").expect("static provider ref is valid")],
                probe_limits: ProbeLimits {
                    max_observation_age: Duration::from_secs(uci.probe_interval_secs),
                },
                scope_valid: runtime_scope_valid(config_valid, &runtime_profile),
                ipv6_ready: true,
                assigned_device_configured: !assigned_device.is_empty(),
                assigned_device: if assigned_device.is_empty() {
                    None
                } else {
                    Some(assigned_device)
                },
                reverse_geo_lookup: Some(("nominatim.openstreetmap.org".to_owned(), 443)),
            },
        )
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
        Box::new(service)
    };

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
    let proxy =
        MitmProxy::new(&mitm_ca, upstream_roots)?.with_events_file(std::path::PathBuf::from(
            std::env::var("WLOC_EVENTS_FILE")
                .unwrap_or_else(|_| "/var/run/wloc-service/events.jsonl".into()),
        ));
    // The upstream connection must use the real Apple IP (the DNS hijack
    // would otherwise point it back at this router). Prefer the first
    // nft-set address; fall back to DNS-only resolution when the set is
    // empty (e.g. rules not yet installed).
    let proxy = match upstream_apple_ips().into_iter().next() {
        Some(apple_ip) => {
            eprintln!("wloc-service: upstream apple ip override {apple_ip}:443");
            proxy.with_upstream_override(apple_ip, 443)
        }
        None => proxy,
    };
    let proxy = std::sync::Arc::new(proxy);
    let proxy_port: u16 = env_or("WLOC_PROXY_PORT", 8443_u16);

    // Apply the persisted manual location before serving. Enabling is deferred
    // until both the control socket and proxy listener are bound, so the
    // runtime health gate never has to assume a listener that does not exist.
    if profile_model
        .as_ref()
        .is_none_or(|model| model.profiles().len() <= 1)
        && runtime_profile.location_mode == LocationMode::Manual
    {
        if let (Some(latitude), Some(longitude)) = (
            runtime_profile.manual_latitude,
            runtime_profile.manual_longitude,
        ) {
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

    // Proxy TLS health: whether the test devices trust the current root CA.
    // A TLS handshake failure (untrusted CA, wrong SNI, ...) is recorded so
    // the admin UI can show "certificate not trusted" instead of a mystery.
    let proxy_health = std::sync::Arc::new(std::sync::Mutex::new(ProxyHealth::default()));
    let health_path = std::env::var("WLOC_HEALTH_FILE")
        .unwrap_or_else(|_| "/var/run/wloc-service/proxy-health.json".into());

    // TPROXY listener: IP_TRANSPARENT lets the kernel deliver connections
    // whose original destination is a remote host (the Apple WLOC IP), so
    // iOS sees a perfectly normal connection to the Apple server - unlike
    // REDIRECT, which rewrites the destination to this router and newer iOS
    // versions answer with RST.
    let proxy_listener = runtime.block_on(async { bind_tproxy_listener(proxy_port) })?;
    let service_pid_path =
        profile_marker_path("WLOC_SERVICE_PIDFILE", "/var/run/wloc-service/service.pid");
    let _service_pid_marker = write_service_pid_marker(&service_pid_path)?;
    if profile_model
        .as_ref()
        .is_none_or(|model| model.profiles().len() <= 1)
        && runtime_profile.enabled
    {
        match service.enable() {
            Ok(()) => {
                if let Some(router) = profile_router.as_ref() {
                    if let Err(error) = router.set_enabled(&runtime_profile.id, true) {
                        eprintln!("wloc-service: enabling profile route failed: {error:?}");
                    }
                }
            }
            Err(error) => eprintln!("wloc-service: enable failed: {error:?}"),
        }
    }
    let multi_profile_mode = profile_model
        .as_ref()
        .is_some_and(|model| model.profiles().len() > 1);
    if multi_profile_mode {
        let proxy_ready = profile_marker_path(
            "WLOC_PROFILE_PROXY_READY_FILE",
            "/var/run/wloc-service/profiles/.proxy-ready",
        );
        let _ = std::fs::remove_file(&proxy_ready);
        write_profile_marker(&proxy_ready)?;
        if std::env::var("WLOC_SUPERVISED").as_deref() == Ok("1") {
            let activate = profile_marker_path(
                "WLOC_PROFILE_ACTIVATE_FILE",
                "/var/run/wloc-service/profiles/.activate",
            );
            let timeout = Duration::from_secs(env_or("WLOC_PROFILE_ACTIVATE_TIMEOUT", 30_u64));
            if !wait_for_profile_marker(&activate, timeout) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "supervisor did not authorize profile activation",
                )
                .into());
            }
        }
        // In supervised mode this runs only after the supervisor has passed
        // its child health gate, installed the shared route, and published
        // the activation marker. Standalone mode still activates only after
        // the listener is bound, never during daemon construction.
        service.activate_profiles();
    }
    runtime.spawn(async move {
        loop {
            if let Ok((stream, _)) = proxy_listener.accept().await {
                let proxy = proxy.clone();
                let patch_state = std::sync::Arc::clone(&patch_state);
                let profile_router = profile_router.clone();
                let proxy_health = std::sync::Arc::clone(&proxy_health);
                let health_path = health_path.clone();
                tokio::spawn(async move {
                    let result = if let Some(router) = profile_router.as_ref() {
                        proxy
                            .handle_connection_routed(stream, router.as_ref())
                            .await
                    } else {
                        let patch = patch_state.lock().ok().and_then(|guard| *guard);
                        proxy.handle_connection(stream, patch.as_ref()).await
                    };
                    match result {
                        Ok(()) => {
                            record_proxy_health(
                                &proxy_health,
                                std::path::Path::new(&health_path),
                                true,
                            );
                        }
                        Err(error) => {
                            eprintln!("wloc proxy: connection error: {error}");
                            record_proxy_health(
                                &proxy_health,
                                std::path::Path::new(&health_path),
                                false,
                            );
                        }
                    }
                });
            }
        }
    });
    eprintln!("wloc-service MITM proxy listening on 0.0.0.0:{proxy_port}");

    eprintln!("wloc-service listening on {socket_path}");
    let profile_dispatch: Box<dyn ProfileDispatch> = if config_valid {
        let uci_binary =
            std::env::var("WLOC_UCI_BINARY").unwrap_or_else(|_| "/sbin/uci".to_owned());
        match UciProfileStore::from_config(&uci, uci_binary) {
            Ok(store) => Box::new(store),
            Err(error) => {
                eprintln!("wloc-service: profile control plane unavailable: {error:?}");
                Box::new(NoProfileDispatch)
            }
        }
    } else {
        Box::new(NoProfileDispatch)
    };
    let server = ControlServer::with_profile_dispatch(service, profile_dispatch);
    // Housekeeping runs every 10s so a node switch in the WLOC settings
    // is followed within seconds. The probe itself only runs when the
    // config fingerprint changed or cached evidence is stale (the check is
    // a cheap file read + hash); the observation age is still governed by
    // probe_interval_secs.
    runtime.block_on(server.serve(listener, std::time::Duration::from_secs(10)));
    Ok(())
}

/// Read the real Apple WLOC IPs from the nft apple_hosts set. The DNS
/// hijack forces the devices to connect locally, but the proxy's own
/// upstream connection must reach the real Apple server - resolving the
/// hostname through dnsmasq would loop back to this router.
fn upstream_apple_ips() -> Vec<String> {
    let output = std::process::Command::new("nft")
        .args(["list", "set", "inet", "wloc_service", "apple_hosts"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let router_ip = lan_router_ip();
    parse_apple_ips(&output)
        .into_iter()
        .filter(|ip| Some(ip.as_str()) != router_ip.as_deref())
        .collect()
}

/// The router's own LAN IPv4 (`uci network.lan.ipaddr`), used to filter the
/// hijacked address out of the upstream Apple IP set on any subnet.
fn lan_router_ip() -> Option<String> {
    let output = std::process::Command::new("uci")
        .args(["-q", "get", "network.lan.ipaddr"])
        .output()
        .ok()?;
    let ip = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if ip.is_empty() {
        None
    } else {
        Some(ip)
    }
}

/// Extract IPv4 addresses from the `nft list set` output (elements line).
fn parse_apple_ips(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("elements") {
                Some(trimmed)
            } else {
                None
            }
        })
        .flat_map(|line| {
            line.split(['{', '}', ',', ' '])
                .map(str::trim)
                .filter(|token| {
                    !token.is_empty()
                        && token.chars().all(|c| c.is_ascii_digit() || c == '.')
                        && token.matches('.').count() == 3
                        && *token != "127.0.0.1"
                })
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
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

    #[test]
    fn parses_apple_ips_from_nft_output() {
        let output = "\nelements = { 59.82.17.33, 140.205.31.96 }\n";
        assert_eq!(
            parse_apple_ips(output),
            vec!["59.82.17.33", "140.205.31.96"]
        );
    }

    #[test]
    fn empty_nft_output_yields_no_ips() {
        assert!(parse_apple_ips("table inet wloc_service {\n}").is_empty());
    }

    #[test]
    fn ignores_non_ip_tokens() {
        let output = "elements = { 59.82.17.33, hostname, 10.0.0.1/8 }\n";
        assert_eq!(parse_apple_ips(output), vec!["59.82.17.33"]);
    }

    #[test]
    fn explicit_profile_is_the_single_runtime_source() {
        let config = WlocUciConfig::parse(
            "config wloc-service 'main'\n\toption enabled '0'\n\toption node_ref 'legacy'\n\toption assigned_device '192.168.1.200'\nconfig device 'phone'\n\toption label 'Phone'\n\toption assigned_device '192.168.1.100'\n\toption node_ref 'profile-node'\n\toption geo_source 'auto'\n",
        )
        .unwrap();
        let profile = runtime_profile_from_uci(&config);
        assert_eq!(profile.id, "phone");
        assert!(profile.enabled);
        assert_eq!(profile.assigned_device.as_deref(), Some("192.168.1.100"));
        assert_eq!(profile.node_ref, "profile-node");
    }

    #[test]
    fn multiple_profiles_never_select_one_implicitly() {
        let config = WlocUciConfig::parse(
            "config device 'phone'\n\toption assigned_device '192.168.1.100'\nconfig device 'tablet'\n\toption assigned_device '192.168.1.101'\n",
        )
        .unwrap();
        let profile = runtime_profile_from_uci(&config);
        assert!(!profile.enabled);
        assert_eq!(profile.id, "invalid");
    }

    #[test]
    fn mac_profile_is_rejected_before_runtime_activation() {
        let config = WlocUciConfig::parse(
            "config device 'phone'\n\toption assigned_device 'aa:bb:cc:dd:ee:ff'\n",
        );
        assert!(matches!(
            config,
            Err(wificalling_location_gateway::config::UciError::Profile(_))
        ));
    }

    #[test]
    fn invalid_uci_cannot_be_enabled_from_the_control_socket() {
        let profile = disabled_runtime_profile();
        assert!(!runtime_scope_valid(false, &profile));
    }

    #[test]
    fn openwrt_runtime_health_does_not_assume_the_engine_is_alive() {
        let mut runtime = OpenWrtRuntime::new("/bin/false", "/bin/false");

        assert!(!RuntimeControl::engine_healthy(&mut runtime).unwrap());
        assert!(!ProfileRuntimeControl::shared_engine_healthy(&mut runtime).unwrap());
    }

    #[test]
    fn profile_handler_runtime_tracks_its_logical_redirect_lifecycle() {
        let mut runtime = ProfileServiceRuntime::default();
        assert!(!runtime.redirect_present().unwrap());
        runtime.install_exact_redirect().unwrap();
        assert!(runtime.redirect_present().unwrap());
        runtime.remove_redirect().unwrap();
        assert!(!runtime.redirect_present().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn openwrt_runtime_delegates_only_component_redirect_actions() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "wloc-runtime-test-{}-{}",
            std::process::id(),
            now_unix()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let log = root.join("actions.log");
        let helper = root.join("redirect-helper.sh");
        let script = format!("#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\n", log.display());
        std::fs::write(&helper, script).unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();

        let mut runtime = OpenWrtRuntime::new(&helper, &helper);
        runtime.defer_first_redirect();
        runtime.install_exact_redirect().unwrap();
        runtime.remove_redirect().unwrap();
        runtime.install_exact_redirect().unwrap();

        assert_eq!(std::fs::read_to_string(&log).unwrap(), "stop\nstart\n");
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn openwrt_runtime_health_accepts_its_socket_and_proxy_listener() {
        use std::net::TcpListener;
        use std::os::unix::net::UnixListener;

        let root = std::env::temp_dir().join(format!(
            "wloc-health-runtime-test-{}-{}",
            std::process::id(),
            now_unix()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let control_socket = root.join("control.sock");
        let _control_listener = UnixListener::bind(&control_socket).unwrap();
        let proxy_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_port = proxy_listener.local_addr().unwrap().port();
        let mut runtime = OpenWrtRuntime::new("/bin/false", "/bin/false")
            .with_health_paths(&control_socket, proxy_port);

        assert!(RuntimeControl::engine_healthy(&mut runtime).unwrap());
        assert!(ProfileRuntimeControl::shared_engine_healthy(&mut runtime).unwrap());

        drop(proxy_listener);
        drop(_control_listener);
        let _ = std::fs::remove_file(&control_socket);
        assert!(!RuntimeControl::engine_healthy(&mut runtime).unwrap());
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn openwrt_runtime_delegates_profile_scoped_redirect_actions() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "wloc-profile-runtime-test-{}-{}",
            std::process::id(),
            now_unix()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let log = root.join("actions.log");
        let helper = root.join("profile-redirect-helper.sh");
        let script = format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\n", log.display());
        std::fs::write(&helper, script).unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();

        let mut runtime = OpenWrtRuntime::new(&helper, &helper);
        let profile = wificalling_location_gateway::config::DeviceProfile {
            id: "phone".to_owned(),
            label: "Phone".to_owned(),
            assigned_device: Some("192.168.1.100".to_owned()),
            node_ref: "default".to_owned(),
            node_mode: wificalling_location_gateway::config::NodeSelectionMode::Fixed,
            location_mode: LocationMode::Auto,
            manual_latitude: None,
            manual_longitude: None,
            manual_location_ref: None,
            enabled: true,
        };
        wificalling_location_gateway::service::profile_runtime::ProfileRuntimeControl::install_profile_redirect(
            &mut runtime,
            &profile,
        )
        .unwrap();
        wificalling_location_gateway::service::profile_runtime::ProfileRuntimeControl::remove_profile_redirect(
            &mut runtime,
            "phone",
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&log).unwrap(),
            "start phone 192.168.1.100\nstop phone\n"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    struct MockProfileDispatch;

    impl ServiceDispatch for MockProfileDispatch {
        fn status(&mut self) -> Result<serde_json::Value, DispatchError> {
            Ok(serde_json::json!({"profile": "mock"}))
        }

        fn enable(&mut self) -> Result<(), DispatchError> {
            Ok(())
        }

        fn disable(&mut self) -> Result<(), DispatchError> {
            Ok(())
        }

        fn reload(&mut self) -> Result<(), DispatchError> {
            Ok(())
        }

        fn set_manual_location(&mut self, _params: &RequestParams) -> Result<(), DispatchError> {
            Ok(())
        }

        fn clear_manual_location(&mut self) -> Result<(), DispatchError> {
            Ok(())
        }

        fn search_location(&mut self, query: &str) -> Result<serde_json::Value, DispatchError> {
            Ok(serde_json::json!({"query": query}))
        }

        fn refresh_periodic(&mut self) {}

        fn refresh_evidence(&mut self) -> Result<(), DispatchError> {
            Ok(())
        }
    }

    #[cfg(unix)]
    #[test]
    fn profile_service_group_coordinates_shared_runtime_and_handlers() {
        use std::net::TcpListener;
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;

        let model = ProfileModel::new(vec![
            DeviceProfile {
                id: "phone".to_owned(),
                label: "Phone".to_owned(),
                assigned_device: Some("192.168.1.100".to_owned()),
                node_ref: "phone-node".to_owned(),
                node_mode: wificalling_location_gateway::config::NodeSelectionMode::Fixed,
                location_mode: LocationMode::Auto,
                manual_latitude: None,
                manual_longitude: None,
                manual_location_ref: None,
                enabled: true,
            },
            DeviceProfile {
                id: "tablet".to_owned(),
                label: "Tablet".to_owned(),
                assigned_device: Some("192.168.1.101".to_owned()),
                node_ref: "tablet-node".to_owned(),
                node_mode: wificalling_location_gateway::config::NodeSelectionMode::Fixed,
                location_mode: LocationMode::Auto,
                manual_latitude: None,
                manual_longitude: None,
                manual_location_ref: None,
                enabled: true,
            },
        ])
        .unwrap();
        let router = std::sync::Arc::new(ProfilePatchRouter::new(&model).unwrap());
        let mut handlers = HashMap::new();
        handlers.insert(
            "phone".to_owned(),
            Box::new(MockProfileDispatch) as BoxedProfileService,
        );
        handlers.insert(
            "tablet".to_owned(),
            Box::new(MockProfileDispatch) as BoxedProfileService,
        );

        let root = std::env::temp_dir().join(format!(
            "wloc-profile-group-test-{}-{}",
            std::process::id(),
            now_unix()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let helper = root.join("profile-redirect-helper.sh");
        let script = format!(
            "#!/bin/sh\nmarker='{}'/$2\ncase \"$1\" in\nstart) touch \"$marker\";;\nstop) rm -f \"$marker\";;\nstatus) test -f \"$marker\";;\nesac\n",
            root.display()
        );
        std::fs::write(&helper, script).unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let control_socket = root.join("control.sock");
        let _control_listener = UnixListener::bind(&control_socket).unwrap();
        let proxy_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy_port = proxy_listener.local_addr().unwrap().port();

        let mut group = ProfileServiceGroup::new(
            model,
            std::sync::Arc::clone(&router),
            handlers,
            OpenWrtRuntime::new(&helper, &helper).with_health_paths(&control_socket, proxy_port),
        );
        group.activate_enabled_profiles();
        assert_eq!(group.runtime.statuses().len(), 2);
        assert!(group
            .runtime
            .statuses()
            .iter()
            .all(|status| status.phase
                == wificalling_location_gateway::service::profile_runtime::ProfileRuntimePhase::Intercepting));
        router
            .set_target("phone", Some(PatchTarget::new(1.0, 2.0)))
            .unwrap();
        assert_eq!(
            router.resolve_source("192.168.1.100").unwrap(),
            PatchTarget::new(1.0, 2.0)
        );

        assert_eq!(group.status().unwrap()["profile"], "mock");
        group.reload().unwrap();
        group
            .set_manual_location(&RequestParams {
                query: None,
                latitude: Some(1.0),
                longitude: Some(2.0),
            })
            .unwrap();
        group.clear_manual_location().unwrap();
        assert_eq!(
            group.search_location("Singapore").unwrap()["query"],
            "Singapore"
        );
        group.refresh_periodic();
        group.refresh_evidence().unwrap();

        group.disable().unwrap();
        assert!(router.resolve_source("192.168.1.100").is_none());
        group.enable().unwrap();
        assert!(router.resolve_source("192.168.1.100").is_none());

        for error in [
            ProfileRuntimeError::EngineStart,
            ProfileRuntimeError::EngineUnhealthy,
            ProfileRuntimeError::RedirectInstall,
            ProfileRuntimeError::RedirectStillPresent,
            ProfileRuntimeError::CleanupUnsafe,
            ProfileRuntimeError::UnsupportedDevice,
            ProfileRuntimeError::ProfileDisabled,
            ProfileRuntimeError::UnknownProfile,
        ] {
            let mapped = map_profile_runtime_error(error);
            assert!(!mapped.wire_code().is_empty());
        }

        let _ = std::fs::remove_dir_all(root);
    }
}
