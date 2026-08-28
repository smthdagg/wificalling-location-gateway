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
HOSTS="gs-loc.apple.com gs-loc-cn.apple.com gsp-ssl.ls.apple.com bluedot.is.autonavi.com bluedot.is.autonavi.com.gds.alibabadns.com gspe19-cn-ssl-ls-apple-com.v.aaplimg.com"
DNS_CONF=/etc/dnsmasq.conf
DNS_MARKER='# wloc-service DNS hijack (do not edit)'

valid_ipv4() {
    case "$1" in ''|*[!0-9.]*|*..*|.*|*.) return 1;; esac
    awk -F. 'NF == 4 { for (i = 1; i <= 4; i++) if ($i !~ /^[0-9]+$/ || $i > 255) exit 1; exit 0 } { exit 1 }' <<EOF
$1
EOF
}

restart_passwall_dns() {
    pid_file=/tmp/etc/passwall/acl/default/dnsmasq.pid
    conf=/tmp/etc/passwall/acl/default/dnsmasq.conf
    bin=/tmp/etc/passwall/bin/dnsmasq_default
    [ -s "$pid_file" ] && [ -x "$bin" ] && [ -f "$conf" ] || return 0
    pid=$(cat "$pid_file")
    kill -TERM "$pid" 2>/dev/null || return 0
    sleep 1
    rm -f "$pid_file"
    "$bin" -C "$conf" -x "$pid_file"
}

if [ "$action" = stop ]; then
    dns_changed=0
    grep -F "$DNS_MARKER" "$DNS_CONF" >/dev/null 2>&1 && {
        sed -i "/$DNS_MARKER/,/^# wloc-service end/d" "$DNS_CONF"
        dns_changed=1
    }
    for hosts_file in /etc/hosts /tmp/hosts/wloc-hosts; do
        sed -i "/$DNS_MARKER/,/^# wloc-service end/d" "$hosts_file" 2>/dev/null || true
    done
    router_ip=$(uci -q get network.lan.ipaddr 2>/dev/null || true)
    dns_changed=0
    for host in $HOSTS; do
        entry="/$host/$router_ip"
        uci -q show dhcp.@dnsmasq[0].address 2>/dev/null | grep -F -- "'$entry'" >/dev/null || continue
        uci del_list "dhcp.@dnsmasq[0].address=$entry"
        dns_changed=1
    done
    if [ "$dns_changed" -eq 1 ]; then
        uci commit dhcp
        /etc/init.d/dnsmasq restart
        restart_passwall_dns
    fi
    ip rule del fwmark "$FWMARK" lookup "$ROUTE_TABLE" 2>/dev/null || true
    ip route del local 0.0.0.0/0 dev lo table "$ROUTE_TABLE" 2>/dev/null || true
    ip -6 rule del fwmark "$FWMARK" lookup "$ROUTE_TABLE" 2>/dev/null || true
    ip -6 route del local ::/0 dev lo table "$ROUTE_TABLE" 2>/dev/null || true
    nft delete table inet "$TABLE" 2>/dev/null || true
    rm -f /var/run/wloc-service/upstream-map.tmp /var/run/wloc-service/upstream-map /var/run/wloc-service/ipv6-scope-ready
    exit 0
fi

# WLOC is scoped to the device selected in its own settings. The Gateway
# first-device fallback preserves fresh-install behavior when the WLOC option
# has not been saved yet; it must never expand scope to every Gateway device.
ips=$(uci -q get wloc-service.main.assigned_device 2>/dev/null || true)
if [ -z "$ips" ]; then
    ips=$(uci -q get wificalling-gateway.@device[0].source_ip 2>/dev/null || true)
fi

[ -n "$ips" ] || {
    echo "wloc-redirect-sync: no devices in the gateway device policy" >&2
    exit 1
}
for ip in $ips; do
    valid_ipv4 "$ip" || { echo "wloc-redirect-sync: invalid device IPv4: $ip" >&2; exit 1; }
done

# The router's LAN IPv4 is the verified WLOC ingress used by the stable r1
# path. Firewall scope below limits TPROXY to the selected WLOC device.
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
if [ -z "$ROUTER_IP" ] || ! valid_ipv4 "$ROUTER_IP"; then
    echo "wloc-redirect-sync: cannot determine the router LAN IP" >&2
    exit 1
fi
case "$PROXY_PORT" in
    ''|*[!0-9]*) echo "wloc-redirect-sync: invalid proxy port" >&2; exit 1;;
esac
if [ "$PROXY_PORT" -lt 1 ] || [ "$PROXY_PORT" -gt 65535 ]; then
    echo "wloc-redirect-sync: proxy port out of range" >&2
    exit 1
fi

# Apple rotates CDN addresses independently for each resolver and device.
# The common config file is loaded by both the system and PassWall dnsmasq;
# the TPROXY rules below still scope the local answer to one device.
dns_changed=0
for host in $HOSTS; do
    entry="/$host/$ROUTER_IP"
    uci -q show dhcp.@dnsmasq[0].address 2>/dev/null | grep -F -- "'$entry'" >/dev/null || continue
    uci del_list "dhcp.@dnsmasq[0].address=$entry"
    dns_changed=1
done
if ! grep -F "$DNS_MARKER" "$DNS_CONF" >/dev/null 2>&1 ||
    ! grep -F "address=/gsp-ssl.ls.apple.com/$ROUTER_IP" "$DNS_CONF" >/dev/null 2>&1; then
    sed -i "/$DNS_MARKER/,/^# wloc-service end/d" "$DNS_CONF"
    {
        printf '%s\n' "$DNS_MARKER"
        for host in $HOSTS; do
            printf 'address=/%s/%s\n' "$host" "$ROUTER_IP"
        done
        printf '%s\n' '# wloc-service end'
    } >> "$DNS_CONF"
    dns_changed=1
fi
if [ "$dns_changed" -eq 1 ]; then
    uci commit dhcp
    /etc/init.d/dnsmasq restart
    restart_passwall_dns
fi

valid_mac() {
    awk -F: 'NF == 6 && $0 !~ /[^0-9A-Fa-f:]/ { exit 0 } { exit 1 }' <<EOF
$1
EOF
}

mac_for_ip() {
    _wloc_ip=$1
    _wloc_mac=$(awk -v target="$_wloc_ip" '$3 == target { print $2; exit }' /tmp/dhcp.leases 2>/dev/null || true)
    if ! valid_mac "$_wloc_mac"; then
        _wloc_mac=$(ip neigh show "$_wloc_ip" dev br-lan 2>/dev/null | awk '$2 == "lladdr" { print $3; exit }')
    fi
    valid_mac "$_wloc_mac" && printf '%s' "$_wloc_mac"
}

macs=
for ip in $ips; do
    mac=$(mac_for_ip "$ip" || true)
    [ -n "$mac" ] && macs="$macs $mac"
done

[ "$action" = prepare ] && exit 0

# TPROXY plumbing: marked packets are routed back to the local stack.
ip rule del fwmark "$FWMARK" lookup "$ROUTE_TABLE" 2>/dev/null || true
ip route del local 0.0.0.0/0 dev lo table "$ROUTE_TABLE" 2>/dev/null || true
ip -6 rule del fwmark "$FWMARK" lookup "$ROUTE_TABLE" 2>/dev/null || true
ip -6 route del local ::/0 dev lo table "$ROUTE_TABLE" 2>/dev/null || true
ip rule add fwmark "$FWMARK" lookup "$ROUTE_TABLE"
ip route add local 0.0.0.0/0 dev lo table "$ROUTE_TABLE"
nft add table inet "$TABLE" 2>/dev/null || true
nft add set inet "$TABLE" apple_hosts '{ type ipv4_addr; }' 2>/dev/null || true
nft add set inet "$TABLE" apple_hosts6 '{ type ipv6_addr; }' 2>/dev/null || true
nft flush chain inet "$TABLE" "$CHAIN" 2>/dev/null || true
nft delete chain inet "$TABLE" "$CHAIN" 2>/dev/null || true
nft "add chain inet $TABLE $CHAIN { type filter hook prerouting priority mangle; }"
for ip in $ips; do
    nft "add rule inet $TABLE $CHAIN ip saddr $ip tcp dport 443 ip daddr @apple_hosts meta l4proto tcp meta mark set $FWMARK tproxy ip to :$PROXY_PORT"
    nft "add rule inet $TABLE $CHAIN ip saddr $ip tcp dport 443 ip daddr $ROUTER_IP meta l4proto tcp meta mark set $FWMARK tproxy ip to :$PROXY_PORT"
done
for mac in $macs; do
    nft "add rule inet $TABLE $CHAIN ether saddr $mac tcp dport 443 ip6 daddr @apple_hosts6 reject with tcp reset"
done
echo "wloc-redirect-sync: IPv4 tproxy $ips -> :$PROXY_PORT (mark $FWMARK, table $ROUTE_TABLE)"
