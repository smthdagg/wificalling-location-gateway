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

HOSTS="gs-loc.apple.com gs-loc-cn.apple.com bluedot.is.autonavi.com bluedot.is.autonavi.com.gds.alibabadns.com"

collect() {
    # Query an explicit public resolver so the DNS hijack (which maps the
    # WLOC names to 192.168.31.1) does not pollute the set.
    for host in $HOSTS; do
        nslookup "$host" 223.5.5.5 2>/dev/null \
            | sed -n 's/^Address: *\([0-9][0-9.]*\)$/\1/p'
        nslookup "$host" 119.29.29.29 2>/dev/null \
            | sed -n 's/^Address: *\([0-9][0-9.]*\)$/\1/p'
    done
}

ips=$(collect | grep -v '^192.168.31.1$' | sort -u | tr '
' ',' | sed 's/,\$$//')
[ -n "$ips" ] || {
    echo "wloc-refresh-set: no A records resolved (DNS unavailable?)" >&2
    exit 1
}

nft flush set inet "$TABLE" "$SET" 2>/dev/null || nft add set inet "$TABLE" "$SET" '{ type ipv4_addr; }'
nft add element inet "$TABLE" "$SET" "{ $ips }"
echo "wloc-refresh-set: updated $SET = { $ips }"
