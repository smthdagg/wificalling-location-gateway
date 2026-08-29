//! Real sing-box exit probe.
//!
//! Reads the Wi-Fi Calling Gateway's running sing-box configuration
//! (`/var/run/wificalling-gateway/sing-box.json`), finds the outbound bound to
//! the assigned test device, and asks an IP echo service through the matching
//! loopback probe inbound already owned by the running Gateway sing-box.
//!
//! Parsing and port selection are pure functions tested offline; network I/O
//! runs through the existing Gateway listener on the router.

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use serde_json::Value;

use super::runtime::{ExitProbeRuntime, ProbeFailure};
use super::ExitProbeError;

/// A parsed sing-box outbound (we only need tag + the raw object to re-emit).
#[derive(Clone, Debug)]
pub struct SingBoxOutbound {
    pub tag: String,
}

/// A parsed sing-box wireguard endpoint (sing-box 1.13+ keeps wireguard
/// peers in `endpoints`, not `outbounds`).
#[derive(Clone, Debug)]
pub struct SingBoxEndpoint {
    pub tag: String,
}

/// The parsed configuration pieces the probe needs.
#[derive(Clone, Debug)]
pub struct SingBoxConfig {
    pub outbounds: Vec<SingBoxOutbound>,
    pub endpoints: Vec<SingBoxEndpoint>,
}

/// Select the outbound the route rules bind to `device_ip`.
pub fn select_outbound_tag(document: &Value, device_ip: IpAddr) -> Option<String> {
    let rules = document.get("route")?.get("rules")?.as_array()?;
    rules.iter().find_map(|rule| {
        let cidrs = rule.get("source_ip_cidr")?.as_array()?;
        let matches = cidrs.iter().any(|cidr| {
            cidr.as_str()
                .and_then(|text| text.split('/').next())
                .and_then(|ip| ip.parse::<IpAddr>().ok())
                .map(|ip| ip == device_ip)
                .unwrap_or(false)
        });
        if matches {
            rule.get("outbound")
                .and_then(Value::as_str)
                .map(String::from)
        } else {
            None
        }
    })
}

/// Resolve the node bound to `device_ip` in the Gateway device-policy UCI
/// text (`/etc/config/wificalling-gateway`), returning its `node-<section>`
/// tag. Disabled policies are included: the follow-device IP is defined by
/// the bound node, not by whether Wi-Fi Calling interception is enabled.
pub fn device_bound_node_tag(uci_text: &str, device_ip: IpAddr) -> Option<String> {
    let mut in_device = false;
    let mut source_ips: Vec<String> = Vec::new();
    let mut node: Option<String> = None;
    for raw_line in uci_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("config ") {
            // Flush the previous device section before starting a new one
            // (the next `config device` must not swallow it).
            if in_device
                && source_ips
                    .iter()
                    .any(|ip| ip.parse::<IpAddr>().ok() == Some(device_ip))
            {
                return node.map(|section| format!("node-{section}"));
            }
            in_device = line.starts_with("config device");
            source_ips.clear();
            node = None;
            continue;
        }
        if !in_device {
            continue;
        }
        if let Some(value) = option_value(line, "option node") {
            node = Some(value);
        } else if let Some(value) = option_value(line, "list source_ip") {
            source_ips.push(value);
        }
    }
    if in_device
        && source_ips
            .iter()
            .any(|ip| ip.parse::<IpAddr>().ok() == Some(device_ip))
    {
        return node.map(|section| format!("node-{section}"));
    }
    None
}

/// Resolve the outbound tag bound to `device_ip`: the UCI device-policy
/// binding first (the user's source of truth - the running sing-box rules
/// may lag a node switch), then the sing-box route rule.
pub fn select_node_tag(
    document: &Value,
    uci_text: Option<&str>,
    device_ip: IpAddr,
) -> Option<String> {
    if let Some(uci_text) = uci_text {
        if let Some(tag) = device_bound_node_tag(uci_text, device_ip) {
            return Some(tag);
        }
    }
    select_outbound_tag(document, device_ip)
}

/// Extract the quoted value of a UCI `option`/`list` line, e.g.
/// `option node 'cfg1146ab'` -> `cfg1146ab`.
fn option_value(line: &str, keyword: &str) -> Option<String> {
    let rest = line.strip_prefix(keyword)?.trim_start();
    let value = rest.strip_prefix('\'').or_else(|| rest.strip_prefix('"'))?;
    let end = value.find(['\'', '"'])?;
    Some(value[..end].to_owned())
}

/// Parse a sing-box.json document, retaining outbounds and wireguard
/// endpoints for reuse.
pub fn parse_singbox_config(document: &Value) -> SingBoxConfig {
    let outbounds = document
        .get("outbounds")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let tag = item.get("tag")?.as_str()?.to_owned();
                    Some(SingBoxOutbound { tag })
                })
                .collect()
        })
        .unwrap_or_default();
    let endpoints = document
        .get("endpoints")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let tag = item.get("tag")?.as_str()?.to_owned();
                    Some(SingBoxEndpoint { tag })
                })
                .collect()
        })
        .unwrap_or_default();
    SingBoxConfig {
        outbounds,
        endpoints,
    }
}

/// Find the loopback HTTP inbound compiled for an existing node. The Gateway
/// owns these listeners; WLOC must reuse one instead of starting a probe
/// sing-box process of its own.
pub fn existing_probe_port(document: &Value, node_tag: &str) -> Option<u16> {
    let node_id = node_tag
        .strip_prefix("node-")
        .or_else(|| node_tag.strip_prefix("wg-"))?;
    let expected_tag = format!("probe-{node_id}");
    document
        .get("inbounds")
        .and_then(Value::as_array)?
        .iter()
        .find(|inbound| {
            inbound.get("type").and_then(Value::as_str) == Some("http")
                && inbound.get("tag").and_then(Value::as_str) == Some(expected_tag.as_str())
                && inbound.get("listen").and_then(Value::as_str) == Some("127.0.0.1")
        })
        .and_then(|inbound| inbound.get("listen_port").and_then(Value::as_u64))
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| (1024..=65535).contains(port))
}

/// Upper bound for the probe HTTP response (headers + body).
const MAX_PROBE_RESPONSE_BYTES: usize = 16 * 1024;

/// Ask an IP echo service through the local HTTP proxy and return the exit IP.
///
/// The request uses an absolute-form URI (RFC 9110 §3.2.2) against the
/// Gateway's loopback probe inbound, with bounded connect/read timeouts and a
/// response size cap so the control path can never hang or balloon. Pure
/// std::net keeps the probe free of external command dependencies (a clean
/// OpenWrt image does not ship curl).
fn query_exit_ip(probe_port: u16, timeout: Duration) -> Result<IpAddr, ProbeFailure> {
    use std::io::{Read, Write};
    let address = std::net::SocketAddr::from((Ipv4Addr::LOCALHOST, probe_port));
    let mut stream = std::net::TcpStream::connect_timeout(&address, timeout)
        .map_err(|_| ProbeFailure::Unreachable)?;
    stream
        .set_read_timeout(Some(timeout))
        .and_then(|()| stream.set_write_timeout(Some(timeout)))
        .map_err(|_| ProbeFailure::Unreachable)?;
    let request = "GET http://ip-api.com/json?fields=query HTTP/1.1\r\n\
                   Host: ip-api.com\r\nAccept: application/json\r\nConnection: close\r\n\r\n";
    stream
        .write_all(request.as_bytes())
        .map_err(|_| ProbeFailure::Unreachable)?;

    let mut raw = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                raw.extend_from_slice(&buffer[..count]);
                if raw.len() > MAX_PROBE_RESPONSE_BYTES {
                    return Err(ProbeFailure::InvalidData);
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(ProbeFailure::Timeout);
            }
            Err(_) => return Err(ProbeFailure::Unreachable),
        }
    }

    let body = probe_http_body(&raw)?;
    let value: Value = serde_json::from_slice(body).map_err(|_| ProbeFailure::InvalidData)?;
    let ip = value
        .get("query")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<IpAddr>().ok())
        .ok_or(ProbeFailure::InvalidData)?;
    Ok(ip)
}

/// Split the probe response into its HTTP body, requiring a 200 status.
fn probe_http_body(raw: &[u8]) -> Result<&[u8], ProbeFailure> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(ProbeFailure::InvalidData)?;
    let status = raw[..header_end]
        .split(|byte| *byte == b' ')
        .nth(1)
        .and_then(|part| std::str::from_utf8(part).ok())
        .and_then(|part| part.trim().parse::<u16>().ok())
        .ok_or(ProbeFailure::InvalidData)?;
    if status != 200 {
        return Err(ProbeFailure::InvalidData);
    }
    Ok(&raw[header_end + 4..])
}

/// Parse non-loopback IPv4 addresses from `ip -4 addr show` output.
pub fn parse_wan_ips(text: &str) -> Vec<IpAddr> {
    let mut addresses = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(inet) = line.strip_prefix("inet ") {
            if let Some(cidr) = inet.split_whitespace().next() {
                if let Some(ip) = cidr
                    .split('/')
                    .next()
                    .and_then(|s| s.parse::<IpAddr>().ok())
                {
                    if ip != IpAddr::V4(Ipv4Addr::UNSPECIFIED)
                        && ip != IpAddr::V4(Ipv4Addr::LOCALHOST)
                        && !ip.is_loopback()
                    {
                        addresses.push(ip);
                    }
                }
            }
        }
    }
    addresses
}

/// A probe backed by the Gateway's sing-box outbounds.
pub struct SingBoxProbe {
    config_path: PathBuf,
    device_ip: IpAddr,
    timeout: Duration,
    /// Gateway device-policy UCI file (test-injectable). Used to resolve
    /// the bound node for devices that have no route rule (e.g. disabled
    /// Wi-Fi Calling policies), so follow-device still probes their node.
    uci_config_path: PathBuf,
}

impl SingBoxProbe {
    pub fn new(config_path: PathBuf, device_ip: IpAddr) -> Self {
        Self {
            config_path,
            device_ip,
            timeout: Duration::from_secs(15),
            uci_config_path: PathBuf::from("/etc/config/wificalling-gateway"),
        }
    }

    /// Read the Gateway config and select the outbound for the test device.
    fn load_outbound_tag(&self) -> Result<String, ProbeFailure> {
        let text =
            std::fs::read_to_string(&self.config_path).map_err(|_| ProbeFailure::Unreachable)?;
        let document: Value = serde_json::from_str(&text).map_err(|_| ProbeFailure::InvalidData)?;
        let config = parse_singbox_config(&document);

        // 1. The node bound to the device policy in UCI - the source of
        //    truth. The Gateway regenerates its sing-box route rules at its
        //    own pace, so a fresh UCI binding must win over a stale rule.
        //    The node may be a regular outbound, or a wireguard endpoint
        //    which the Gateway compiler names `wg-<section>` (sing-box
        //    1.11+), while the UCI binding resolves to `node-<section>`.
        let uci_text = std::fs::read_to_string(&self.uci_config_path).ok();
        let selected_tag = if let Some(uci_text) = uci_text.as_deref() {
            // A readable UCI file is authoritative. If the followed device
            // or its bound node was deleted, a stale sing-box route must not
            // resurrect it and an unrelated first node must never be used.
            device_bound_node_tag(uci_text, self.device_ip).ok_or(ProbeFailure::BoundNodeMissing)?
        } else {
            // Compatibility for pre-UCI/test environments: the running route
            // may identify the device, but absence still fails closed.
            select_outbound_tag(&document, self.device_ip).ok_or(ProbeFailure::BoundNodeMissing)?
        };

        {
            let tag = selected_tag;
            if config.outbounds.iter().any(|o| o.tag == tag) {
                return Ok(tag);
            }
            if let Some(section) = tag.strip_prefix("node-") {
                let endpoint_tag = format!("wg-{section}");
                if config.endpoints.iter().any(|e| e.tag == endpoint_tag) {
                    return Ok(endpoint_tag);
                }
            }
            if config.endpoints.iter().any(|e| e.tag == tag) {
                return Ok(tag);
            }
        }
        Err(ProbeFailure::BoundNodeMissing)
    }

    /// Probe the node's real exit IP through the running Gateway sing-box.
    fn probe_with_node(&self, outbound_tag: &str) -> Result<IpAddr, ProbeFailure> {
        let text =
            std::fs::read_to_string(&self.config_path).map_err(|_| ProbeFailure::Unreachable)?;
        let document: Value = serde_json::from_str(&text).map_err(|_| ProbeFailure::InvalidData)?;
        let probe_port =
            existing_probe_port(&document, outbound_tag).ok_or(ProbeFailure::BoundNodeMissing)?;
        query_exit_ip(probe_port, self.timeout)
    }
}

impl ExitProbeRuntime for SingBoxProbe {
    fn probe_exit_ip(&mut self) -> Result<IpAddr, ProbeFailure> {
        let outbound_tag = self.load_outbound_tag()?;
        self.probe_with_node(&outbound_tag)
    }

    fn router_wan_ips(&mut self) -> Result<Vec<IpAddr>, ProbeFailure> {
        // Collect WAN addresses from the system routes via `ip -4 addr`.
        let output = Command::new("ip")
            .args(["-4", "addr", "show"])
            .output()
            .map_err(|_| ProbeFailure::Unreachable)?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut addresses = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if let Some(inet) = line.strip_prefix("inet ") {
                if let Some(cidr) = inet.split_whitespace().next() {
                    if let Some(ip) = cidr
                        .split('/')
                        .next()
                        .and_then(|s| s.parse::<IpAddr>().ok())
                    {
                        if ip != IpAddr::V4(Ipv4Addr::UNSPECIFIED)
                            && ip != IpAddr::V4(Ipv4Addr::LOCALHOST)
                            && !ip.is_loopback()
                        {
                            addresses.push(ip);
                        }
                    }
                }
            }
        }
        if addresses.is_empty() {
            // Fail-open boundary: no WAN address known means observations are
            // rejected by the validation layer (RouterWanUnknown).
            return Ok(vec![]);
        }
        Ok(addresses)
    }

    /// FNV-1a over the Gateway sing-box config AND the device-policy UCI
    /// text: a node switch can change either the running rule set or the
    /// UCI binding, and both must trigger an immediate re-probe even while
    /// cached evidence is still fresh.
    fn config_fingerprint(&mut self) -> Option<u64> {
        let mut hash = fnv1a(&std::fs::read(&self.config_path).ok()?);
        if let Ok(uci_bytes) = std::fs::read(&self.uci_config_path) {
            hash = fnv1a_continue(hash, &uci_bytes);
        }
        Some(hash)
    }
}

/// FNV-1a over `bytes`.
fn fnv1a(bytes: &[u8]) -> u64 {
    fnv1a_continue(0xcbf29ce484222325, bytes)
}

/// Continue an FNV-1a hash over `bytes`.
fn fnv1a_continue(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[allow(dead_code)]
fn _keep_error_link(error: ExitProbeError) -> ExitProbeError {
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_document() -> Value {
        json!({
            "outbounds": [
                {"type": "vless", "tag": "node-hk", "server": "1.2.3.4", "server_port": 443},
                {"type": "direct", "tag": "direct"}
            ],
            "route": {
                "rules": [
                    {"source_ip_cidr": ["192.168.31.175/32"], "action": "route", "outbound": "node-hk"}
                ]
            }
        })
    }

    #[test]
    fn existing_probe_port_is_selected_for_the_followed_node() {
        let document = json!({
            "inbounds": [
                {"type": "http", "tag": "probe-a", "listen": "127.0.0.1", "listen_port": 23456},
                {"type": "http", "tag": "probe-b", "listen": "127.0.0.1", "listen_port": 24567}
            ]
        });
        assert_eq!(existing_probe_port(&document, "node-a"), Some(23456));
        assert_eq!(existing_probe_port(&document, "wg-b"), Some(24567));
        assert_eq!(existing_probe_port(&document, "node-missing"), None);
    }

    #[test]
    fn device_outbound_is_selected_from_route_rules() {
        let doc = sample_document();
        assert_eq!(
            select_outbound_tag(&doc, IpAddr::V4(Ipv4Addr::new(192, 168, 31, 175))),
            Some("node-hk".to_owned())
        );
        assert_eq!(
            select_outbound_tag(&doc, IpAddr::V4(Ipv4Addr::new(192, 168, 31, 176))),
            None
        );
    }

    #[test]
    fn uci_wireguard_binding_resolves_to_an_endpoint_tag() {
        // A device bound to a wireguard node has no matching outbound, but
        // the endpoint (named `wg-<section>` by the Gateway compiler) must
        // be selected for the follow-device probe.
        let doc = json!({
            "outbounds": [{"type": "direct", "tag": "direct"}],
            "endpoints": [{"type": "wireguard", "tag": "wg-wgtest", "address": ["10.0.0.1/24"]}]
        });
        let dir = std::env::temp_dir();
        let config_path = dir.join("wloc-singbox-wg-endpoint.json");
        let uci_path = dir.join("wloc-singbox-wg-uci");
        std::fs::write(&config_path, doc.to_string()).unwrap();
        std::fs::write(
            &uci_path,
            "config device\n\toption label 'iPhone17'\n\toption node 'wgtest'\n\tlist source_ip '192.168.31.176'\n",
        )
        .unwrap();
        let mut probe = SingBoxProbe::new(
            config_path.clone(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 31, 176)),
        );
        probe.uci_config_path = uci_path.clone();
        // Selection resolves to the wireguard endpoint tag; the sing-box
        // spawn then fails on this host, which maps to Unreachable - the
        // important assertion is that load_outbound_tag() does not give up.
        assert_eq!(probe.load_outbound_tag(), Ok("wg-wgtest".to_owned()));
        std::fs::remove_file(&config_path).unwrap();
        std::fs::remove_file(&uci_path).unwrap();
        let _ = std::fs::remove_dir_all(dir.join("wloc-singbox-wg-endpoint-work"));
    }

    #[test]
    fn query_exit_ip_parses_the_echo_field() {
        // The parsing path is exercised indirectly via a tiny inline check:
        // ip-api returns {"query":"1.2.3.4"}; parse that shape.
        let value: Value = serde_json::from_str(r#"{"query":"1.2.3.4"}"#).unwrap();
        let ip = value
            .get("query")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<IpAddr>().ok())
            .unwrap();
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)));
    }

    #[test]
    fn missing_existing_probe_inbound_fails_closed() {
        let doc = json!({"outbounds": [
            {"type": "hysteria2", "tag": "node-a"},
            {"type": "direct", "tag": "direct"}
        ], "route": {"rules": [
            {"source_ip_cidr": ["192.168.31.176/32"], "outbound": "node-a"}
        ]}});
        let dir = std::env::temp_dir();
        let config_path = dir.join("wloc-singbox-fallback.json");
        std::fs::write(&config_path, doc.to_string()).unwrap();
        let mut probe = SingBoxProbe::new(
            config_path.clone(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 31, 176)),
        );
        assert_eq!(probe.probe_exit_ip(), Err(ProbeFailure::BoundNodeMissing));
        std::fs::remove_file(&config_path).unwrap();
        let _ = std::fs::remove_dir_all(dir.join("wloc-singbox-fallback-work"));
    }

    #[test]
    fn query_exit_ip_fails_when_the_proxy_is_down() {
        // No listener on this port; the connect fails -> Unreachable.
        assert_eq!(
            query_exit_ip(59_999, Duration::from_millis(500)),
            Err(ProbeFailure::Unreachable)
        );
    }

    /// One-shot mock HTTP proxy: captures the request line, answers `status`.
    fn spawn_mock_probe_proxy(
        status_line: &str,
        body: &str,
    ) -> (u16, std::sync::Arc<std::sync::Mutex<String>>) {
        use std::net::TcpListener;
        use std::sync::{Arc, Mutex};
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let captured: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let captured_in_thread = Arc::clone(&captured);
        let status_line = status_line.to_owned();
        let body = body.to_owned();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut request = String::new();
                let mut buffer = [0_u8; 4096];
                if let Ok(count) = stream.read(&mut buffer) {
                    request.push_str(&String::from_utf8_lossy(&buffer[..count]));
                }
                *captured_in_thread.lock().unwrap() = request;
                let response = format!(
                    "{status_line}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        (port, captured)
    }

    #[test]
    fn query_exit_ip_parses_the_proxied_exit_over_an_absolute_form_uri() {
        let (port, captured) =
            spawn_mock_probe_proxy("HTTP/1.1 200 OK", r#"{"query":"203.0.113.7"}"#);
        assert_eq!(
            query_exit_ip(port, Duration::from_secs(5)),
            Ok(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)))
        );
        // The probe inbound is an HTTP proxy: the target must be requested in
        // absolute-form so the Gateway sing-box routes it through the node.
        let request = captured.lock().unwrap().clone();
        assert!(
            request.starts_with("GET http://ip-api.com/json?fields=query HTTP/1.1\r\n"),
            "probe request must use an absolute-form URI, got: {request:?}"
        );
    }

    #[test]
    fn query_exit_ip_rejects_non_200_probe_answers() {
        let (port, _captured) =
            spawn_mock_probe_proxy("HTTP/1.1 502 Bad Gateway", "upstream unavailable");
        assert_eq!(
            query_exit_ip(port, Duration::from_secs(5)),
            Err(ProbeFailure::InvalidData)
        );
    }

    #[test]
    fn parse_wan_ips_skips_loopback_and_link_local() {
        let text = "1: lo: <LOOPBACK>\n    inet 127.0.0.1/8 scope host lo\n2: eth0: <UP>\n    inet 192.168.31.1/24 brd 192.168.31.255 scope global eth0\n    inet 10.0.0.5/8 scope global eth0\n";
        let ips = parse_wan_ips(text);
        assert!(ips.contains(&IpAddr::V4(Ipv4Addr::new(192, 168, 31, 1))));
        assert!(ips.contains(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))));
        assert!(!ips.contains(&IpAddr::V4(Ipv4Addr::LOCALHOST)));
    }

    #[test]
    fn device_bound_node_tag_parses_the_device_policy() {
        // The iPhone17 policy is disabled (enabled=0) so it has no route
        // rule, but its bound node must still be probeable - the follow-
        // device IP comes from that node's exit.
        let uci = r#"
config wificalling-gateway 'main'
	option enabled '1'

config device
	option label 'iPhone12'
	option route_mode 'independent'
	option node 'cfg0a46ab'
	list source_ip '192.168.31.175'

config device
	option label 'iPhone17'
	option route_mode 'independent'
	option node 'cfg1146ab'
	option enabled '0'
	list source_ip '192.168.31.176'
"#;
        assert_eq!(
            device_bound_node_tag(uci, IpAddr::V4(Ipv4Addr::new(192, 168, 31, 176))),
            Some("node-cfg1146ab".to_owned())
        );
        assert_eq!(
            device_bound_node_tag(uci, IpAddr::V4(Ipv4Addr::new(192, 168, 31, 175))),
            Some("node-cfg0a46ab".to_owned())
        );
        assert_eq!(
            device_bound_node_tag(uci, IpAddr::V4(Ipv4Addr::new(192, 168, 31, 99))),
            None
        );
        assert_eq!(
            device_bound_node_tag("", IpAddr::V4(Ipv4Addr::new(192, 168, 31, 176))),
            None
        );
    }

    #[test]
    fn select_node_tag_prefers_uci_binding_over_route_rules() {
        // The route rule binds the device to node-a, but the UCI policy
        // binds it to node-cfg0b. The UCI binding is the user's source of
        // truth and must win: the Gateway regenerates its sing-box rules
        // at its own pace, so a stale rule must not override a fresh
        // binding (regression for node-switch not being followed).
        let doc = json!({
            "outbounds": [
                {"type": "vless", "tag": "node-a", "server": "1.2.3.4", "server_port": 443},
                {"type": "vless", "tag": "node-cfg0b", "server": "5.6.7.8", "server_port": 443},
                {"type": "direct", "tag": "direct"}
            ],
            "route": {"rules": [
                {"source_ip_cidr": ["192.168.31.175/32"], "action": "route", "outbound": "node-a"}
            ]}
        });
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 31, 175));
        let uci = "config device\n\toption label 'iPhone12'\n\toption node 'cfg0b'\n\tlist source_ip '192.168.31.175'\n";
        assert_eq!(
            select_node_tag(&doc, Some(uci), ip),
            Some("node-cfg0b".to_owned())
        );
        // Without a UCI binding the route rule is used.
        assert_eq!(select_node_tag(&doc, None, ip), Some("node-a".to_owned()));
        // A UCI binding for a different device does not shadow the rule.
        let other = "config device\n\toption label 'iPhone17'\n\toption node 'cfg0c'\n\tlist source_ip '192.168.31.176'\n";
        assert_eq!(
            select_node_tag(&doc, Some(other), ip),
            Some("node-a".to_owned())
        );
    }

    #[test]
    fn config_fingerprint_changes_when_the_uci_binding_changes() {
        // The fingerprint must cover the device-policy UCI text as well as
        // the sing-box config: a node switch that only rewrites the binding
        // (without regenerating the running rule set) still triggers an
        // immediate re-probe.
        let dir = std::env::temp_dir();
        let config_path = dir.join("wloc-singbox-fp.json");
        let uci_path = dir.join("wloc-singbox-fp.uci");
        std::fs::write(&config_path, sample_document().to_string()).unwrap();
        let mut probe = SingBoxProbe::new(
            config_path.clone(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 31, 175)),
        );
        probe.uci_config_path = uci_path.clone();

        std::fs::write(
            &uci_path,
            "config device\n\toption node 'cfg0a46ab'\n\tlist source_ip '192.168.31.175'\n",
        )
        .unwrap();
        let first = probe.config_fingerprint();
        std::fs::write(
            &uci_path,
            "config device\n\toption node 'cfg1146ab'\n\tlist source_ip '192.168.31.175'\n",
        )
        .unwrap();
        let second = probe.config_fingerprint();
        assert!(first.is_some() && second.is_some());
        assert_ne!(first, second);

        // The sing-box config alone still produces a fingerprint when the
        // UCI file is missing.
        std::fs::remove_file(&uci_path).unwrap();
        assert!(probe.config_fingerprint().is_some());
        std::fs::remove_file(&config_path).unwrap();
        let _ = std::fs::remove_dir_all(dir.join("wloc-singbox-fp-work"));
    }

    #[test]
    fn load_outbound_tag_prefers_device_bound_node_over_fallback() {
        // No route rule for the device, but the device policy binds a node
        // that exists in the sing-box outbounds: probe must use it, not the
        // arbitrary first outbound.
        let doc = json!({"outbounds": [
            {"type": "hysteria2", "tag": "node-a"},
            {"type": "vless", "tag": "node-cfg1146ab"},
            {"type": "direct", "tag": "direct"}
        ]});
        let dir = std::env::temp_dir();
        let config_path = dir.join("wloc-singbox-bound-node.json");
        let uci_path = dir.join("wloc-singbox-bound-node.uci");
        std::fs::write(&config_path, doc.to_string()).unwrap();
        std::fs::write(
            &uci_path,
            "config device
\toption label 'iPhone17'
\toption node 'cfg1146ab'
\tlist source_ip '192.168.31.176'
",
        )
        .unwrap();
        let mut probe = SingBoxProbe::new(
            config_path.clone(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 31, 176)),
        );
        probe.uci_config_path = uci_path.clone();
        // load_outbound_tag must select the bound node tag; the sing-box
        // spawn then fails on this host, which is fine.
        let _ = probe.probe_exit_ip();
        std::fs::remove_file(&config_path).unwrap();
        std::fs::remove_file(&uci_path).unwrap();
        let _ = std::fs::remove_dir_all(dir.join("wloc-singbox-bound-node-work"));
    }

    #[test]
    fn deleted_bound_node_never_falls_back_to_an_unrelated_outbound() {
        let doc = json!({"outbounds": [
            {"type": "vmess", "tag": "node-unrelated"},
            {"type": "direct", "tag": "direct"}
        ]});
        let dir = std::env::temp_dir();
        let suffix = std::process::id();
        let config_path = dir.join(format!("wloc-deleted-node-{suffix}.json"));
        let uci_path = dir.join(format!("wloc-deleted-node-{suffix}.uci"));
        std::fs::write(&config_path, doc.to_string()).unwrap();
        std::fs::write(
            &uci_path,
            "config device\n\toption node 'deleted'\n\tlist source_ip '192.168.31.175'\n",
        )
        .unwrap();
        let mut probe = SingBoxProbe::new(
            config_path.clone(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 31, 175)),
        );
        probe.uci_config_path = uci_path.clone();

        assert_eq!(
            probe.load_outbound_tag(),
            Err(ProbeFailure::BoundNodeMissing)
        );

        std::fs::remove_file(config_path).unwrap();
        std::fs::remove_file(uci_path).unwrap();
    }

    #[test]
    fn deleted_device_policy_never_uses_a_stale_runtime_route() {
        let doc = json!({
            "outbounds": [
                {"type": "vmess", "tag": "node-stale"},
                {"type": "direct", "tag": "direct"}
            ],
            "route": {"rules": [{
                "source_ip_cidr": ["192.168.31.175/32"],
                "action": "route",
                "outbound": "node-stale"
            }]}
        });
        let dir = std::env::temp_dir();
        let suffix = std::process::id();
        let config_path = dir.join(format!("wloc-deleted-device-{suffix}.json"));
        let uci_path = dir.join(format!("wloc-deleted-device-{suffix}.uci"));
        std::fs::write(&config_path, doc.to_string()).unwrap();
        std::fs::write(
            &uci_path,
            "config device\n\toption node 'other'\n\tlist source_ip '192.168.31.176'\n",
        )
        .unwrap();
        let mut probe = SingBoxProbe::new(
            config_path.clone(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 31, 175)),
        );
        probe.uci_config_path = uci_path.clone();

        assert_eq!(
            probe.load_outbound_tag(),
            Err(ProbeFailure::BoundNodeMissing)
        );

        std::fs::remove_file(config_path).unwrap();
        std::fs::remove_file(uci_path).unwrap();
    }

    #[test]
    fn load_outbound_tag_reads_the_gateway_config() {
        let dir = std::env::temp_dir();
        let config_path = dir.join("wloc-singbox-load-test.json");
        std::fs::write(&config_path, sample_document().to_string()).unwrap();
        let mut probe = SingBoxProbe::new(
            config_path.clone(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 31, 175)),
        );
        // load_outbound_tag picks node-hk from the route rule; the missing
        // loopback inbound then fails closed.
        let _ = probe.probe_exit_ip();
        std::fs::remove_file(&config_path).unwrap();
        let _ = std::fs::remove_dir_all(dir.join("wloc-singbox-load-work"));
    }
}
