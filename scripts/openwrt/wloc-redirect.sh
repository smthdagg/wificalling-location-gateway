#!/bin/sh
# Install or remove the precise WLOC redirect on OpenWrt.
#
# Only the assigned test device's TCP 443 traffic to the six approved WLOC
# WLOC hostnames is redirected to the local wloc-service MITM proxy. The
# redirect lives in its own `wloc_service` table so the Gateway 1.7 table and
# all other traffic are never touched.

set -eu

TABLE=wloc_service
CHAIN_PREROUTING=prerouting
PROXY_PORT="${WLOC_PROXY_PORT:-8443}"
DEVICE_IP="${WLOC_DEVICE_IP:-}"

[ -n "$DEVICE_IP" ] || {
    echo "wloc-redirect: WLOC_DEVICE_IP is required (the assigned test iPhone IP)" >&2
    exit 1
}

case "$DEVICE_IP" in
    [0-9]*.*) ;;
    *) echo "wloc-redirect: invalid device IP: $DEVICE_IP" >&2; exit 1 ;;
esac

case "${1:-install}" in
    install)
        nft add table inet "$TABLE" 2>/dev/null || true
        nft 'add chain inet '"$TABLE"' '"$CHAIN_PREROUTING"' { type nat hook prerouting priority -100; }' 2>/dev/null || true
        nft "add rule inet $TABLE $CHAIN_PREROUTING ip saddr $DEVICE_IP tcp dport 443 ip daddr gs-loc.apple.com redirect to :$PROXY_PORT" 2>/dev/null || true
        nft "add rule inet $TABLE $CHAIN_PREROUTING ip saddr $DEVICE_IP tcp dport 443 ip daddr gs-loc-cn.apple.com redirect to :$PROXY_PORT" 2>/dev/null || true
        nft "add rule inet $TABLE $CHAIN_PREROUTING ip saddr $DEVICE_IP tcp dport 443 ip daddr gs-loc-corpa.apple.com redirect to :$PROXY_PORT" 2>/dev/null || true
        nft "add rule inet $TABLE $CHAIN_PREROUTING ip saddr $DEVICE_IP tcp dport 443 ip daddr gs-loc.apple.com.cn redirect to :$PROXY_PORT" 2>/dev/null || true
        nft "add rule inet $TABLE $CHAIN_PREROUTING ip saddr $DEVICE_IP tcp dport 443 ip daddr bluedot.is.autonavi.com redirect to :$PROXY_PORT" 2>/dev/null || true
        nft "add rule inet $TABLE $CHAIN_PREROUTING ip saddr $DEVICE_IP tcp dport 443 ip daddr bluedot.is.autonavi.com.gds.alibabadns.com redirect to :$PROXY_PORT" 2>/dev/null || true
        # Note: no output chain. A router-local output redirect would also
        # capture the proxy's own upstream connection to the Apple host and
        # loop it back into the proxy.
        echo "wloc-redirect: installed (device $DEVICE_IP -> :$PROXY_PORT)"
        ;;
    remove)
        nft delete table inet "$TABLE" 2>/dev/null || true
        echo "wloc-redirect: removed"
        ;;
    *)
        echo "usage: $0 install|remove" >&2
        exit 2
        ;;
esac
