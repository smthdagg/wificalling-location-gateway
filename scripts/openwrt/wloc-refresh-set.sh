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

HOSTS="gs-loc.apple.com gs-loc-cn.apple.com gs-loc-corpa.apple.com gs-loc.apple.com.cn bluedot.is.autonavi.com bluedot.is.autonavi.com.gds.alibabadns.com"

# The router's own LAN IPv4 is excluded from the destination set if it appears
# in a resolver answer; a local DNS answer is never treated as Apple data.
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

collect_v4() {
    # Query explicit public resolvers.  The client DNS answer is never used
    # as a trust decision; it only feeds the owned destination set.
    for host in $HOSTS; do
        nslookup -type=A "$host" 223.5.5.5 2>/dev/null \
            | sed -n 's/^Address: *\([0-9][0-9.]*\)$/\1/p'
        nslookup -type=A "$host" 119.29.29.29 2>/dev/null \
            | sed -n 's/^Address: *\([0-9][0-9.]*\)$/\1/p'
    done
}

collect_v6() {
    for host in $HOSTS; do
        nslookup -type=AAAA "$host" 223.5.5.5 2>/dev/null \
            | awk '/^Address:[[:space:]]/ { value=$2; if (value !~ /\./ && value ~ /^[0-9A-Fa-f]*:[0-9A-Fa-f:]+$/) print value }'
        nslookup -type=AAAA "$host" 119.29.29.29 2>/dev/null \
            | awk '/^Address:[[:space:]]/ { value=$2; if (value !~ /\./ && value ~ /^[0-9A-Fa-f]*:[0-9A-Fa-f:]+$/) print value }'
    done
}

csv() {
    tr '\n' ',' | sed 's/,$//'
}

ips4=$(collect_v4 | grep -v "^$ROUTER_IP$" | sort -u | csv)
ips6=$(collect_v6 | awk 'index($0, ":") > 0' | sort -u | csv)
[ -n "$ips4" ] || [ -n "$ips6" ] || {
    echo "wloc-refresh-set: no A/AAAA records resolved (DNS unavailable?)" >&2
    exit 1
}

nft list table inet "$TABLE" >/dev/null 2>&1 || {
    echo "wloc-refresh-set: owned nft table is absent" >&2
    exit 1
}
nft flush set inet "$TABLE" "$SET" 2>/dev/null || nft add set inet "$TABLE" "$SET" '{ type ipv4_addr; }'
nft flush set inet "$TABLE" apple_hosts6 2>/dev/null || nft add set inet "$TABLE" apple_hosts6 '{ type ipv6_addr; }'
[ -n "$ips4" ] && nft add element inet "$TABLE" "$SET" "{ $ips4 }"
[ -n "$ips6" ] && nft add element inet "$TABLE" apple_hosts6 "{ $ips6 }"
mkdir -p /var/run/wloc-service
printf '%s\n' "${ips4%%,*}" > /var/run/wloc-service/upstream-ip.tmp
mv /var/run/wloc-service/upstream-ip.tmp /var/run/wloc-service/upstream-ip
echo "wloc-refresh-set: updated $SET = { $ips4 }; apple_hosts6 = { $ips6 }"
