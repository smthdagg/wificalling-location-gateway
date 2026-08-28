#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
redirect="$repo_root/openwrt/files/usr/sbin/wloc-redirect-sync.sh"
refresh="$repo_root/openwrt/files/usr/sbin/wloc-refresh-set.sh"
service="$repo_root/openwrt/files/etc/init.d/wloc-service"
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
grep -F 'upstream-ip.tmp' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must publish the current upstream target' >&2; exit 1; }
grep -F '/usr/sbin/wloc-refresh-set.sh' "$service" >/dev/null ||
	{ echo 'WLOC init start must populate the initial upstream target' >&2; exit 1; }
grep -F 'START=100' "$service" >/dev/null ||
	{ echo 'WLOC must start after the Gateway sing-box service' >&2; exit 1; }
grep -F '$ROUTER_IP gs-loc.apple.com gs-loc-cn.apple.com gs-loc-corpa.apple.com gs-loc.apple.com.cn bluedot.is.autonavi.com bluedot.is.autonavi.com.gds.alibabadns.com' "$redirect" >/dev/null ||
	{ echo 'DNS hijack must contain exactly six approved hosts' >&2; exit 1; }
grep -F 'valid_ipv4 "$ROUTER_IP"' "$redirect" >/dev/null ||
	{ echo 'DNS hijack must validate the router IPv4 before nft/hosts writes' >&2; exit 1; }
grep -F '[ "$PROXY_PORT" -le 65535 ]' "$redirect" >/dev/null ||
	{ echo 'TPROXY port must be range-validated' >&2; exit 1; }
grep -F 'ipv6_interception_ready()' "$daemon" >/dev/null ||
	{ echo 'daemon must gate IPv6 readiness instead of hard-coding it' >&2; exit 1; }
grep -F 'with_upstream_override_file' "$daemon" >/dev/null ||
	{ echo 'daemon must consume refreshed upstream targets' >&2; exit 1; }

printf 'WLOC runtime contract checks passed\n'
