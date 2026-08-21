#!/bin/sh
# Sync the WLOC TPROXY rules with the Wi-Fi Calling device policies.
#
# Every device listed in the gateway device policy (source_ip) gets its
# TCP 443 traffic to the approved Apple WLOC hosts passed to the local
# wloc-service MITM proxy via TPROXY: the original destination address is
# preserved, so iOS sees a normal connection to the Apple server (REDIRECT
# rewrites the destination to this router, which newer iOS rejects with
# RST). The apple_hosts set is kept (maintained by wloc-refresh-set.sh);
# only the rules/route are rebuilt so device changes take effect at once.

set -eu

TABLE=wloc_service
CHAIN=prerouting
PROXY_PORT="${WLOC_PROXY_PORT:-8443}"
FWMARK=1
ROUTE_TABLE=100
action=${1:-start}
NFT_BINARY=${WLOC_NFT_BINARY:-nft}
IP_BINARY=${WLOC_IP_BINARY:-ip}
PROFILE_HELPER=${WLOC_PROFILE_REDIRECT_HELPER:-/usr/sbin/wloc-profile-redirect.sh}

multiple_profiles_configured() {
	command -v uci >/dev/null 2>&1 || return 1
	profiles=$(uci -q show wloc-service 2>/dev/null \
		| sed -n 's/^wloc-service\.[a-z0-9_-]*=device$/x/p' \
		| wc -l | tr -d ' ')
	[ "${profiles:-0}" -gt 1 ] 2>/dev/null
}

withdraw_legacy_redirect() {
	# Only remove the WLOC-owned table, policy route, and DNS marker. The
	# stable Gateway 1.7 nftables table (and UDP 500/4500 handling) is never
	# touched by this cleanup path.
	"$NFT_BINARY" delete table inet "$TABLE" 2>/dev/null || true
	"$IP_BINARY" rule del fwmark "$FWMARK" lookup "$ROUTE_TABLE" 2>/dev/null || true
	"$IP_BINARY" route del local 0.0.0.0/0 dev lo table "$ROUTE_TABLE" 2>/dev/null || true
	for hosts_file in ${WLOC_HOSTS_FILES:-/etc/hosts\ /tmp/hosts/wloc-hosts}; do
		sed -i '/# wloc-service DNS hijack (do not edit)/,/^# wloc-service end/d' "$hosts_file" 2>/dev/null || true
	done
	}

if [ "$action" = stop ] || [ "$action" = legacy-stop ]; then
	withdraw_legacy_redirect
	if [ "$action" = stop ] && [ -x "$PROFILE_HELPER" ]; then
		"$PROFILE_HELPER" stop-all >/dev/null 2>&1 || true
	fi
	exit 0
fi

if [ "$action" = start ] && multiple_profiles_configured; then
	# The profile dispatcher installs only verified, profile-scoped rules. The
	# legacy all-device table must not be installed alongside it. Its stale
	# table and DNS marker are removed, while the shared policy route remains
	# available to the profile-scoped TPROXY chains.
	withdraw_legacy_redirect
	[ -x "$PROFILE_HELPER" ] || exit 1
	"$PROFILE_HELPER" route-start
	exit 0
fi

# Collect the LAN IPs of every device in the gateway device policy.
ips=$(uci -q show wificalling-gateway \
    | sed -n "s/.*\.source_ip=['\"]*\([0-9][0-9.]*\)['\"]*/\1/p" \
    | sort -u)

[ -n "$ips" ] || {
    echo "wloc-redirect-sync: no devices in the gateway device policy" >&2
    exit 1
}

# The router's own LAN IPv4, used for the DNS hijack and the matching
# TPROXY rule. UCI is authoritative; fall back to the LAN bridge address.
lan_ip() {
    ip=$(uci -q get network.lan.ipaddr) || ip=
    case "$ip" in
        ''|*[!0-9.]*)
            ip=$(ip -4 addr show br-lan 2>/dev/null | sed -n 's/^[[:space:]]*inet \([0-9.]*\)\/.*/\1/p' | head -1)
            ;;
    esac
    printf '%s' "$ip"
}

ROUTER_IP=$(lan_ip)
[ -n "$ROUTER_IP" ] || {
    echo "wloc-redirect-sync: cannot determine the router LAN IP" >&2
    exit 1
}

# DNS hijack: force the Apple WLOC hostnames to this router so the
# devices always connect to an address our rules match, regardless of
# CDN IP rotation (the Apple names resolve to different aliyun/akamai
# ranges per client, so a fixed-IP set alone keeps missing them).
HOSTS_MARKER='# wloc-service DNS hijack (do not edit)'
# dnsmasq reads addn-hosts from the /tmp/hosts directory on this build;
# /etc/hosts is kept as a fallback.
mkdir -p /tmp/hosts
for hosts_file in ${WLOC_HOSTS_FILES:-/etc/hosts\ /tmp/hosts/wloc-hosts}; do
    sed -i "/$HOSTS_MARKER/,/^# wloc-service end/d" "$hosts_file" 2>/dev/null || true
    cat >> "$hosts_file" <<EOF
$HOSTS_MARKER
$ROUTER_IP gs-loc.apple.com gs-loc-cn.apple.com
# wloc-service end
EOF
done

# TPROXY plumbing: marked packets are routed back to the local stack.
"$IP_BINARY" rule del fwmark "$FWMARK" lookup "$ROUTE_TABLE" 2>/dev/null || true
"$IP_BINARY" route del local 0.0.0.0/0 dev lo table "$ROUTE_TABLE" 2>/dev/null || true
"$IP_BINARY" rule add fwmark "$FWMARK" lookup "$ROUTE_TABLE"
"$IP_BINARY" route add local 0.0.0.0/0 dev lo table "$ROUTE_TABLE"

# Table + set + mangle prerouting chain (filter hook, before DNAT).
"$NFT_BINARY" add table inet "$TABLE" 2>/dev/null || true
"$NFT_BINARY" add set inet "$TABLE" apple_hosts '{ type ipv4_addr; flags timeout; timeout 30s; }' 2>/dev/null || true
# The chain must be a filter/mangle chain for tproxy; drop a leftover
# nat chain (from the old redirect scheme) first.
"$NFT_BINARY" flush chain inet "$TABLE" "$CHAIN" 2>/dev/null || true
"$NFT_BINARY" delete chain inet "$TABLE" "$CHAIN" 2>/dev/null || true
"$NFT_BINARY" "add chain inet $TABLE $CHAIN { type filter hook prerouting priority mangle; }"

# Rebuild only the rules; the apple_hosts set content is untouched.
# Match both the DNS-set Apple IPs and the hijacked local address.
"$NFT_BINARY" flush chain inet "$TABLE" "$CHAIN"
for ip in $ips; do
    "$NFT_BINARY" "add rule inet $TABLE $CHAIN ip saddr $ip tcp dport 443 ip daddr @apple_hosts meta l4proto tcp meta mark set $FWMARK tproxy ip to :$PROXY_PORT"
    "$NFT_BINARY" "add rule inet $TABLE $CHAIN ip saddr $ip tcp dport 443 ip daddr $ROUTER_IP meta l4proto tcp meta mark set $FWMARK tproxy ip to :$PROXY_PORT"
done

echo "wloc-redirect-sync: tproxy $ips -> :$PROXY_PORT (mark $FWMARK, table $ROUTE_TABLE)"
