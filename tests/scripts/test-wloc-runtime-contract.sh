#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
redirect="$repo_root/openwrt/files/usr/sbin/wloc-redirect-sync.sh"
refresh="$repo_root/openwrt/files/usr/sbin/wloc-refresh-set.sh"
service="$repo_root/openwrt/files/etc/init.d/wloc-service"
gateway_service="$repo_root/openwrt/files/etc/init.d/wificalling-gateway"
rust="$repo_root/src/lib.rs"
daemon="$repo_root/src/bin/wloc-service.rs"

grep -F 'if [ "${1:-start}" = stop ]; then' "$redirect" >/dev/null ||
	{ echo 'WLOC redirect helper must support explicit teardown' >&2; exit 1; }
grep -F '/usr/sbin/wloc-redirect-sync.sh stop' "$service" >/dev/null ||
	{ echo 'WLOC init stop must remove owned firewall state' >&2; exit 1; }
grep -F 'APPROVED_WLOC_HOSTS: [&str; 6]' "$rust" >/dev/null ||
	{ echo 'Rust interception allowlist must contain exactly six hosts' >&2; exit 1; }
grep -F 'HOSTS="gs-loc.apple.com gs-loc-cn.apple.com gs-loc-corpa.apple.com gs-loc.apple.com.cn bluedot.is.autonavi.com bluedot.is.autonavi.com.gds.alibabadns.com"' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must contain exactly six approved hosts' >&2; exit 1; }
grep -F 'apple_hosts6' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must maintain an IPv6 destination set' >&2; exit 1; }
grep -F 'value !~ /\./' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must ignore resolver IPv4:port lines in AAAA output' >&2; exit 1; }
grep -F 'upstream-ip.tmp' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must publish the current upstream target' >&2; exit 1; }
grep -F '/usr/sbin/wloc-refresh-set.sh' "$service" >/dev/null ||
	{ echo 'WLOC init start must populate the initial upstream target' >&2; exit 1; }
grep -F 'START=100' "$service" >/dev/null ||
	{ echo 'WLOC must start after the Gateway sing-box service' >&2; exit 1; }
sync_line=$(awk '/^start_service\(\)/,/^}/ { if ($0 ~ /wloc-redirect-sync\.sh/) { print NR; exit } }' "$service")
daemon_line=$(awk '/^start_service\(\)/,/^}/ { if ($0 ~ /procd_set_param command \/usr\/sbin\/wloc-service/) { print NR; exit } }' "$service")
[ -n "$sync_line" ] && [ -n "$daemon_line" ] && [ "$sync_line" -lt "$daemon_line" ] ||
	{ echo 'WLOC must create its redirect state before starting the daemon'; exit 1; }
grep -F 'apple_hosts6' "$redirect" >/dev/null ||
	{ echo 'TPROXY rules must contain an IPv6 destination set' >&2; exit 1; }
grep -F 'ip6 daddr @apple_hosts6' "$redirect" >/dev/null ||
	{ echo 'TPROXY rules must match IPv6 WLOC destinations' >&2; exit 1; }
redirect_start=$(sed -n '/^# WLOC is scoped/,$p' "$redirect")
printf '%s\n' "$redirect_start" | grep -F 'ip -6 rule add fwmark' >/dev/null ||
	{ echo 'IPv6 TPROXY rules must install a marked local route' >&2; exit 1; }
printf '%s\n' "$redirect_start" | grep -F 'ip -6 rule del fwmark' >/dev/null ||
	{ echo 'IPv6 TPROXY sync must be idempotent before adding its policy rule' >&2; exit 1; }
grep -F 'ipv6-scope-ready' "$redirect" >/dev/null ||
	{ echo 'redirect sync must publish the scoped IPv6 readiness result' >&2; exit 1; }
grep -F 'ether saddr' "$redirect" >/dev/null ||
	{ echo 'IPv6 TPROXY rules must remain scoped to the assigned device' >&2; exit 1; }
grep -F 'wloc-service.main.assigned_device' "$redirect" >/dev/null ||
	{ echo 'TPROXY source scope must come from the WLOC assigned device' >&2; exit 1; }
if grep -F 'uci -q show wificalling-gateway' "$redirect" >/dev/null; then
	{ echo 'TPROXY must not scope WLOC to every Gateway device policy' >&2; exit 1; }
fi
if grep -F 'cat >> "$hosts_file"' "$redirect" >/dev/null; then
	{ echo 'WLOC must not globally hijack DNS for every LAN device' >&2; exit 1; }
fi
stop_block=$(awk '/^stop_service\(\)/,/^}/' "$gateway_service")
stopped_block=$(awk '/^service_stopped\(\)/,/^}/' "$gateway_service")
printf '%s\n' "$stopped_block" | grep -F 'firewall.sh stop' >/dev/null ||
	{ echo 'Gateway stop cleanup must run after procd has terminated its instances' >&2; exit 1; }
if printf '%s\n' "$stop_block" | grep -F 'firewall.sh stop' >/dev/null; then
	{ echo 'Gateway stop must not race Passwall nft cleanup before procd termination' >&2; exit 1; }
fi
grep -F 'valid_ipv4 "$ROUTER_IP"' "$redirect" >/dev/null ||
	{ echo 'redirect sync must validate the router IPv4 before nft writes' >&2; exit 1; }
grep -F '[ "$PROXY_PORT" -le 65535 ]' "$redirect" >/dev/null ||
	{ echo 'TPROXY port must be range-validated' >&2; exit 1; }
grep -F 'ipv6_scope_ready()' "$daemon" >/dev/null ||
	{ echo 'daemon must gate IPv6 readiness using the scoped runtime capability' >&2; exit 1; }
grep -F 'bind_tproxy_listener_v6' "$daemon" >/dev/null ||
	{ echo 'daemon must listen for IPv6 TPROXY traffic' >&2; exit 1; }
grep -F 'with_upstream_override_file' "$daemon" >/dev/null ||
	{ echo 'daemon must consume refreshed upstream targets' >&2; exit 1; }
if grep -F 'with_upstream_override(apple_ip' "$daemon" >/dev/null ||
	grep -F 'upstream_apple_ips()' "$daemon" >/dev/null; then
	{ echo 'production proxy must not pin TPROXY traffic to the first DNS address' >&2; exit 1; }
fi

printf 'WLOC runtime contract checks passed\n'
