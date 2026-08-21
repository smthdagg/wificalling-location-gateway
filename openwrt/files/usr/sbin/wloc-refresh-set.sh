#!/bin/sh
# Refresh the WLOC redirect set with the current DNS answers for the approved
# Apple WLOC hostnames (and their autonavi CNAME targets).
#
# The Apple names resolve through several CNAME layers whose IPs rotate, so a
# fixed-IP rule goes stale. This script re-resolves all approved names and
# replaces the nftables set, then it can be scheduled (cron) to keep the
# redirect live. The proxy itself still decides by SNI/hostname whether a
# connection is a WLOC request.

set -eu

TABLE=wloc_service
SET=apple_hosts
NFT_BINARY=${WLOC_NFT_BINARY:-nft}

HOSTS="gs-loc.apple.com gs-loc-cn.apple.com"

# The router's own LAN IPv4 (the DNS hijack maps the WLOC names to it);
# answers equal to it must not pollute the set.
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
    echo "wloc-refresh-set: cannot determine the router LAN IP" >&2
    exit 1
}

collect() {
    # Query an explicit public resolver so the DNS hijack (which maps the
    # WLOC names to the router's LAN IP) does not pollute the set.
    for host in $HOSTS; do
        nslookup "$host" 223.5.5.5 2>/dev/null \
            | sed -n 's/^Address: *\([0-9][0-9.]*\)$/\1/p'
        nslookup "$host" 119.29.29.29 2>/dev/null \
            | sed -n 's/^Address: *\([0-9][0-9.]*\)$/\1/p'
    done
}

ips=$(collect | grep -v "^$ROUTER_IP$" | sort -u | tr '
' ',' | sed 's/,\$$//')
[ -n "$ips" ] || {
    echo "wloc-refresh-set: no A records resolved (DNS unavailable?)" >&2
    exit 1
}

"$NFT_BINARY" flush set inet "$TABLE" "$SET" 2>/dev/null || \
    "$NFT_BINARY" add set inet "$TABLE" "$SET" '{ type ipv4_addr; }'
"$NFT_BINARY" add element inet "$TABLE" "$SET" "{ $ips }"

# V2 profiles each have an isolated nft table and set. Refresh the same
# approved Apple answers into every live profile table without touching the
# stable Gateway namespace. Table names are validated before being reused as
# nft arguments even though they originate from the local kernel listing.
profile_tables=$(
    "$NFT_BINARY" list tables inet 2>/dev/null \
        | sed -n 's/^table inet \(wloc_profile_[a-z0-9_-]*\)$/\1/p' \
        || true
)
while IFS= read -r profile_table; do
    [ -n "$profile_table" ] || continue
    case "$profile_table" in
        wloc_profile_*|*[!a-z0-9_-]*)
            case "$profile_table" in
                *[!a-z0-9_-]*) continue ;;
            esac
            ;;
        *) continue ;;
    esac
    "$NFT_BINARY" flush set inet "$profile_table" "$SET" 2>/dev/null || \
        "$NFT_BINARY" add set inet "$profile_table" "$SET" '{ type ipv4_addr; }'
    "$NFT_BINARY" add element inet "$profile_table" "$SET" "{ $ips }"
done <<EOF
$profile_tables
EOF
echo "wloc-refresh-set: updated $SET = { $ips }"
