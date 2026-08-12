#!/bin/sh
# Sync the WLOC redirect rules with the Wi-Fi Calling device policies.
#
# Every device listed in the gateway's device policy (source_ip) gets its
# TCP 443 traffic to the approved Apple WLOC hosts redirected to the local
# wloc-service MITM proxy. The apple_hosts set is kept (it is maintained by
# wloc-refresh-set.sh against rotating DNS answers); only the rules in the
# prerouting chain are rebuilt, so adding/removing/changing the followed
# device takes effect immediately.

set -eu

TABLE=wloc_service
CHAIN=prerouting
PROXY_PORT="${WLOC_PROXY_PORT:-8443}"

# Collect the LAN IPs of every device in the gateway device policy.
ips=$(uci -q show wificalling-gateway \
    | sed -n "s/.*\.source_ip=['\"]*\([0-9][0-9.]*\)['\"]*/\1/p" \
    | sort -u)

[ -n "$ips" ] || {
    echo "wloc-redirect-sync: no devices in the gateway device policy" >&2
    exit 1
}

# Ensure the table, set and chain exist (first install).
nft add table inet "$TABLE" 2>/dev/null || true
nft add set inet "$TABLE" apple_hosts '{ type ipv4_addr; }' 2>/dev/null || true
nft "add chain inet $TABLE $CHAIN { type nat hook prerouting priority -100; }" 2>/dev/null || true

# Rebuild only the rules; the apple_hosts set content is untouched.
nft flush chain inet "$TABLE" "$CHAIN"
for ip in $ips; do
    nft "add rule inet $TABLE $CHAIN ip saddr $ip tcp dport 443 ip daddr @apple_hosts counter redirect to :$PROXY_PORT"
done

echo "wloc-redirect-sync: redirecting $ips -> :$PROXY_PORT"
