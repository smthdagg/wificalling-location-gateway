#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
redirect="$repo_root/openwrt/files/usr/sbin/wloc-redirect-sync.sh"
refresh="$repo_root/openwrt/files/usr/sbin/wloc-refresh-set.sh"
service="$repo_root/openwrt/files/etc/init.d/wloc-service"
gateway_service="$repo_root/openwrt/files/etc/init.d/wificalling-gateway"
rust="$repo_root/src/lib.rs"
daemon="$repo_root/src/bin/wloc-service.rs"

grep -F 'if [ "$action" = stop ]; then' "$redirect" >/dev/null ||
	{ echo 'WLOC redirect helper must support explicit teardown' >&2; exit 1; }
grep -F '/usr/sbin/wloc-redirect-sync.sh stop' "$service" >/dev/null ||
	{ echo 'WLOC init stop must remove owned firewall state' >&2; exit 1; }
grep -F 'APPROVED_WLOC_HOSTS: [&str; 6]' "$rust" >/dev/null ||
	{ echo 'Rust interception allowlist must contain exactly six hosts' >&2; exit 1; }
grep -F 'HOSTS="gs-loc.apple.com gs-loc-cn.apple.com gsp-ssl.ls.apple.com bluedot.is.autonavi.com bluedot.is.autonavi.com.gds.alibabadns.com gspe19-cn-ssl-ls-apple-com.v.aaplimg.com"' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must contain exactly six approved hosts' >&2; exit 1; }
grep -F 'wloc_service4 4 apple_hosts' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must maintain an IPv4 dnsmasq nftset section' >&2; exit 1; }
grep -F 'wloc_service6 6 apple_hosts6' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must maintain an IPv6 deny-set section' >&2; exit 1; }
grep -F '"family=$family"' "$refresh" >/dev/null ||
	{ echo 'dnsmasq entries must set their address family outside the set name' >&2; exit 1; }
grep -F '"table=wloc_service"' "$refresh" >/dev/null ||
	{ echo 'dnsmasq entries must use the owned nftables table' >&2; exit 1; }
grep -F 'uci add_list "dhcp.$section.name=$set_name"' "$refresh" >/dev/null ||
	{ echo 'dnsmasq entries must add the client IPv4 answer to the owned set' >&2; exit 1; }
grep -F 'uci delete dhcp.wloc_service' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must migrate the invalid combined nftset section' >&2; exit 1; }
grep -F 'uci del_list' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must replace only stale WLOC dnsmasq entries' >&2; exit 1; }
grep -F '/etc/init.d/dnsmasq restart' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must activate changed dnsmasq nftset entries' >&2; exit 1; }
grep -F 'dnsmasq --help' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must fall back safely when dnsmasq lacks nftset support' >&2; exit 1; }
grep -F 'upstream_map_tmp=' "$refresh" >/dev/null ||
	{ echo 'DNS refresh must publish host-specific upstream targets' >&2; exit 1; }
grep -F '/usr/sbin/wloc-refresh-set.sh' "$service" >/dev/null ||
	{ echo 'WLOC init start must populate the initial upstream target' >&2; exit 1; }
grep -F 'START=100' "$service" >/dev/null ||
	{ echo 'WLOC must start after the Gateway sing-box service' >&2; exit 1; }
grep -F 'reload_service() { restart; }' "$service" >/dev/null ||
	{ echo 'WLOC must restart when its persisted configuration changes' >&2; exit 1; }
grep -F 'procd_add_reload_trigger wloc-service wificalling-gateway' "$service" >/dev/null ||
	{ echo 'WLOC must reload when either its scope or Gateway device policy changes' >&2; exit 1; }
# Both branches (enabled and disabled) start the daemon; the enabled branch
# must still populate DNS targets before its procd instance. Compare the LAST
# occurrence of each so the disabled branch's early daemon start cannot mask
# the ordering.
sync_line=$(awk '/wloc-refresh-set\.sh/ { line = NR } END { print line }' "$service")
daemon_line=$(awk '/procd_set_param command \/usr\/sbin\/wloc-service/ { line = NR } END { print line }' "$service")
[ -n "$sync_line" ] && [ -n "$daemon_line" ] && [ "$sync_line" -lt "$daemon_line" ] ||
	{ echo 'WLOC must prepare DNS targets before starting the daemon'; exit 1; }
grep -F 'wloc-redirect-sync.sh prepare' "$service" >/dev/null ||
	{ echo 'WLOC init must prepare its IPv4 scope without installing TPROXY'; exit 1; }
# Disabled WLOC must be genuinely fail-open: the init gates interception-side
# state on the UCI switch, cleans any leftover on start, and the daemon always
# runs so the control API (and the enable toggle) stays reachable.
grep -F 'uci -q get wloc-service.main.enabled' "$service" >/dev/null ||
	{ echo 'WLOC init must gate interception-side state on the enabled switch' >&2; exit 1; }
grep -F 'wloc-redirect-sync.sh stop >/dev/null' "$service" >/dev/null ||
	{ echo 'WLOC init must withdraw leftovers when disabled' >&2; exit 1; }
# Every TPROXY install must carry a fresh upstream map: without it the MITM
# resolves the hijacked ingress back to this router and every request fails
# while status still shows intercepting.
refresh_line=$(awk '/wloc-refresh-set\.sh/{ print NR; exit }' "$redirect")
tproxy_line=$(awk '/^# TPROXY plumbing/{ print NR; exit }' "$redirect")
[ -n "$refresh_line" ] && [ -n "$tproxy_line" ] && [ "$refresh_line" -lt "$tproxy_line" ] ||
	{ echo 'redirect install must refresh the upstream map before TPROXY rules' >&2; exit 1; }
grep -F 'upstream refresh failed; not installing tproxy' "$redirect" >/dev/null ||
	{ echo 'redirect install must fail closed when the upstream map cannot be refreshed' >&2; exit 1; }
# The stop path must remember that removing the DNS hijack block requires a
# dnsmasq restart: a second dns_changed=0 reset swallowed exactly that signal
# and left the running resolver answering with the router IP after disable.
stop_block=$(sed -n '/\$action" = stop/,/^fi$/p' "$redirect")
[ "$(printf '%s\n' "$stop_block" | grep -c 'dns_changed=0')" -eq 1 ] ||
	{ echo 'redirect stop must not reset dns_changed after removing the DNS hijack' >&2; exit 1; }
printf '%s\n' "$stop_block" | grep -F 'dns_changed=1' >/dev/null ||
	{ echo 'redirect stop must flag DNS changes for the dnsmasq restart' >&2; exit 1; }
redirect_start=$(sed -n '/^# WLOC is scoped/,$p' "$redirect")
if grep -F 'ip -6 rule add fwmark' "$redirect" >/dev/null ||
	grep -F 'tproxy ip6' "$redirect" >/dev/null; then
	{ echo 'WLOC must not install an IPv6 interception path' >&2; exit 1; }
fi
grep -F 'ip6 daddr @apple_hosts6 reject with tcp reset' "$redirect" >/dev/null ||
	{ echo 'WLOC must reject only approved IPv6 targets so clients fall back to IPv4' >&2; exit 1; }
grep -F 'wloc-service.main.assigned_device' "$redirect" >/dev/null ||
	{ echo 'TPROXY source scope must come from the WLOC assigned device' >&2; exit 1; }
if grep -F 'uci -q show wificalling-gateway' "$redirect" >/dev/null; then
	{ echo 'TPROXY must not scope WLOC to every Gateway device policy' >&2; exit 1; }
fi
grep -F "printf 'address=/%s/%s" "$redirect" >/dev/null ||
	{ echo 'WLOC must restore the proven local DNS ingress' >&2; exit 1; }
grep -F 'ip daddr $ROUTER_IP' "$redirect" >/dev/null ||
	{ echo 'WLOC must route the local DNS ingress only for its assigned device' >&2; exit 1; }
stop_block=$(awk '/^stop_service\(\)/,/^}/' "$gateway_service")
stopped_block=$(awk '/^service_stopped\(\)/,/^}/' "$gateway_service")
printf '%s\n' "$stopped_block" | grep -F 'firewall.sh stop' >/dev/null ||
	{ echo 'Gateway stop cleanup must run after procd has terminated its instances' >&2; exit 1; }
if printf '%s\n' "$stop_block" | grep -F 'firewall.sh stop' >/dev/null; then
	{ echo 'Gateway stop must not race Passwall nft cleanup before procd termination' >&2; exit 1; }
fi
grep -F 'valid_ipv4 "$ROUTER_IP"' "$redirect" >/dev/null ||
	{ echo 'redirect sync must validate the router IPv4 before nft writes' >&2; exit 1; }
grep -F '[ "$PROXY_PORT" -gt 65535 ]' "$redirect" >/dev/null ||
	{ echo 'TPROXY port must be range-validated' >&2; exit 1; }
if grep -F 'bind_tproxy_listener_v6' "$daemon" >/dev/null ||
	grep -F 'proxy_listener_v6' "$daemon" >/dev/null; then
	{ echo 'daemon must bind only the IPv4 TPROXY listener' >&2; exit 1; }
fi
proxy="$repo_root/src/mitm/proxy.rs"
grep -F 'with_upstream_override_file' "$daemon" >/dev/null ||
	{ echo 'daemon must consume host-specific upstream targets' >&2; exit 1; }
grep -F 'upstream_override_for' "$proxy" >/dev/null ||
	{ echo 'proxy must select the refreshed address by requested hostname' >&2; exit 1; }
if grep -F 'upstream-ip' "$refresh" >/dev/null; then
	{ echo 'WLOC must not publish one shared CDN IP for every hostname' >&2; exit 1; }
fi
grep -F '!ip.is_private()' "$proxy" >/dev/null ||
	{ echo 'local DNS ingress must fall back to its requested hostname' >&2; exit 1; }
if grep -F 'with_upstream_override(apple_ip' "$daemon" >/dev/null ||
	grep -F 'upstream_apple_ips()' "$daemon" >/dev/null; then
	{ echo 'production proxy must not pin TPROXY traffic to the first DNS address' >&2; exit 1; }
fi

printf 'WLOC runtime contract checks passed\n'
