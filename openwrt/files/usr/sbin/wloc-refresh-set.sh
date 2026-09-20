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

HOSTS="gs-loc.apple.com gs-loc-cn.apple.com gsp-ssl.ls.apple.com bluedot.is.autonavi.com bluedot.is.autonavi.com.gds.alibabadns.com gspe19-cn-ssl-ls-apple-com.v.aaplimg.com"

# Ask dnsmasq to add the addresses actually returned to the assigned device.
# A CDN answer from an arbitrary public resolver is only a bootstrap fallback;
# it cannot reliably predict the client's IPv4 or IPv6 answer.
ensure_client_dns_sets() {
    dnsmasq --help 2>&1 | grep -F -- '--nftset' >/dev/null || return 0

    changed=0

    # `family` is deliberately a section option, not part of the set name:
    # OpenWrt's dnsmasq init adds exactly one `4#` or `6#` prefix itself.
    ensure_dnsmasq_section() {
        section=$1 family=$2 set_name=$3
        section_type=$(uci -q get "dhcp.$section" 2>/dev/null || true)
        case "$section_type" in
            '') uci set "dhcp.$section=ipset"; changed=1 ;;
            ipset) ;;
            *) echo "wloc-refresh-set: dhcp.$section is not an ipset section" >&2; return 1 ;;
        esac
        for option_value in "table=wloc_service" "table_family=inet" "family=$family"; do
            option=${option_value%%=*}
            value=${option_value#*=}
            [ "$(uci -q get "dhcp.$section.$option" 2>/dev/null || true)" = "$value" ] || {
                uci set "dhcp.$section.$option=$value"
                changed=1
            }
        done
        uci -q show "dhcp.$section.name" 2>/dev/null | grep -F -- "'$set_name'" >/dev/null || {
            uci add_list "dhcp.$section.name=$set_name"
            changed=1
        }
        # Replace the domain list wholesale, but flag a change only when the
        # current list actually differs from the desired set: refresh runs on
        # every enable/reload and a no-op rewrite must not restart dnsmasq.
        current_domains=$(uci -q get "dhcp.$section.domain" 2>/dev/null || true)
        # shellcheck disable=SC2086  # HOSTS is a space-separated list; word splitting intended
        desired_domains=$(printf '%s\n' $HOSTS | sort)
        if [ "$(printf '%s' "$current_domains" | tr ' ' '\n' | sort)" != "$desired_domains" ]; then
            uci -q delete "dhcp.$section.domain" || true
            for host in $HOSTS; do
                uci add_list "dhcp.$section.domain=$host"
            done
            changed=1
        fi
    }

    # Migration from the first dynamic-DNS attempt, which embedded the family
    # prefix in the set name and therefore generated an invalid nftset target.
    if uci -q get dhcp.wloc_service >/dev/null 2>&1; then
        uci delete dhcp.wloc_service
        changed=1
    fi
    ensure_dnsmasq_section wloc_service4 4 apple_hosts
    ensure_dnsmasq_section wloc_service6 6 apple_hosts6

    # Older builds stored these project-owned rules on the main dnsmasq
    # section, sometimes as one invalid space-joined value. Remove only those
    # known legacy values; all unrelated user nftsets are left untouched.
    legacy_batched='/gs-loc.apple.com/4#inet#wloc_service#apple_hosts /gs-loc-cn.apple.com/4#inet#wloc_service#apple_hosts /bluedot.is.autonavi.com/4#inet#wloc_service#apple_hosts /bluedot.is.autonavi.com.gds.alibabadns.com/4#inet#wloc_service#apple_hosts'
    for legacy in "$legacy_batched" \
        '/gs-loc.apple.com/4#inet#wloc_service#apple_hosts' \
        '/gs-loc-cn.apple.com/4#inet#wloc_service#apple_hosts' \
        '/gs-loc-corpa.apple.com/4#inet#wloc_service#apple_hosts' \
        '/gs-loc.apple.com.cn/4#inet#wloc_service#apple_hosts' \
        '/bluedot.is.autonavi.com/4#inet#wloc_service#apple_hosts' \
        '/bluedot.is.autonavi.com.gds.alibabadns.com/4#inet#wloc_service#apple_hosts'; do
        uci -q show dhcp.@dnsmasq[0].nftset 2>/dev/null | grep -F -- "'$legacy'" >/dev/null || continue
        uci del_list "dhcp.@dnsmasq[0].nftset=$legacy"
        changed=1
    done

    # Keep the stable local ingress owned by wloc-redirect-sync.sh, but clear
    # only retired hostname rules from earlier package revisions.
    for host in gs-loc-corpa.apple.com gs-loc.apple.com.cn; do
        uci -q del_list "dhcp.@dnsmasq[0].address=/$host/$ROUTER_IP" || true
        uci -q del_list "dhcp.@dnsmasq[0].rebind_domain=/$host/" || true
    done

    # PassWall's extra dnsmasq retains its -C configuration across HUP.
    # Remove only the retired local WLOC redirects and restart that helper.
    passwall_changed=0
    for passwall_conf in /tmp/etc/passwall/acl/default/dnsmasq.conf /var/etc/passwall/acl/default/dnsmasq.conf; do
        [ -f "$passwall_conf" ] || continue
        grep -E "^address=/(gs-loc\.apple\.com|gs-loc-cn\.apple\.com|gs-loc\.apple\.com\.cn|gs-loc-corpa\.apple\.com|bluedot\.is\.autonavi\.com(\.gds\.alibabadns\.com)?)/$ROUTER_IP$" "$passwall_conf" >/dev/null || continue
        sed -i "\\|^address=/gs-loc\\.apple\\.com/$ROUTER_IP$|d; \\|^address=/gs-loc-cn\\.apple\\.com/$ROUTER_IP$|d; \\|^address=/gs-loc\\.apple\\.com\\.cn/$ROUTER_IP$|d; \\|^address=/gs-loc-corpa\\.apple\\.com/$ROUTER_IP$|d; \\|^address=/bluedot\\.is\\.autonavi\\.com/$ROUTER_IP$|d; \\|^address=/bluedot\\.is\\.autonavi\\.com\\.gds\\.alibabadns\\.com/$ROUTER_IP$|d" "$passwall_conf"
        passwall_changed=1
    done
    if [ "$passwall_changed" -eq 1 ] && [ -s /tmp/etc/passwall/acl/default/dnsmasq.pid ] && [ -x /tmp/etc/passwall/bin/dnsmasq_default ]; then
        passwall_pid=$(cat /tmp/etc/passwall/acl/default/dnsmasq.pid)
        kill -TERM "$passwall_pid"
        sleep 1
        rm -f /tmp/etc/passwall/acl/default/dnsmasq.pid
        /tmp/etc/passwall/bin/dnsmasq_default -C /tmp/etc/passwall/acl/default/dnsmasq.conf -x /tmp/etc/passwall/acl/default/dnsmasq.pid
    fi

    [ "$changed" -eq 0 ] && return 0
    uci commit dhcp
    /etc/init.d/dnsmasq restart
}

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

# Prefer the resolver(s) supplied by the current WAN/firmware, then retain a
# small public fallback list for networks whose local DNS cannot resolve the
# approved names. Skip the router itself: WLOC's local DNS ingress deliberately
# answers these names with ROUTER_IP, which is not an upstream destination.
resolver_list() {
    {
        awk '$1 == "nameserver" && $2 ~ /^[0-9]+(\.[0-9]+){3}$/ { print $2 }' \
            /tmp/resolv.conf.d/resolv.conf.auto \
            /tmp/resolv.conf.d/resolv.conf \
            /etc/resolv.conf 2>/dev/null || true
        printf '%s\n' 223.5.5.5 119.29.29.29 1.1.1.1
    } | awk 'NF && !seen[$1]++ { print $1 }'
}

DNS_SERVERS=$(resolver_list)
resolve_a() {
    resolve_host=$1
    for resolver in $DNS_SERVERS; do
        answers=$(nslookup -type=A "$resolve_host" "$resolver" 2>/dev/null | awk -v router="$ROUTER_IP" '
            $1 == "Name:" { answer = 1; next }
            answer && $1 == "Address:" && $2 ~ /^[0-9]+(\.[0-9]+){3}$/ && $2 != router { print $2 }
            answer && $1 == "Address" && $2 ~ /^[0-9]+:$/ && $3 ~ /^[0-9]+(\.[0-9]+){3}$/ && $3 != router { print $3 }
        ' | sort -u)
        [ -n "$answers" ] && { printf '%s\n' "$answers"; return 0; }
    done
    return 1
}

ensure_client_dns_sets

collect_v4() {
    # The client DNS answer is never used as a trust decision; this only feeds
    # the owned destination set.
    for host in $HOSTS; do
        resolve_a "$host" || true
    done
}

# AAAA answers feed the @apple_hosts6 reject set: a client that bypasses the
# local DNS hijack (DoH, Private Relay, a hand-edited resolver) still reaches
# the real Apple WLOC servers over IPv6, and this set is what the per-device
# `ip6 daddr @apple_hosts6 reject` guard matches on. Without it the set stays
# empty and the IPv6 fallback guard never fires.
resolve_aaaa() {
    resolve_host=$1
    for resolver in $DNS_SERVERS; do
        answers=$(nslookup -type=AAAA "$resolve_host" "$resolver" 2>/dev/null | awk '
            $1 == "Name:" { answer = 1; next }
            answer && $1 == "Address:" && $2 ~ /^[0-9A-Fa-f:]*:[0-9A-Fa-f:]+$/ { print $2 }
            answer && $1 == "Address" && $3 ~ /^[0-9A-Fa-f:]*:[0-9A-Fa-f:]+$/ { print $3 }
        ' | sort -u)
        [ -n "$answers" ] && { printf '%s\n' "$answers"; return 0; }
    done
    return 1
}

collect_aaaa() {
    for host in $HOSTS; do
        resolve_aaaa "$host" || true
    done
}

csv() {
    tr '\n' ',' | sed 's/,$//'
}

ips4=$(collect_v4 | grep -v "^$ROUTER_IP$" | sort -u | csv)
[ -n "$ips4" ] || {
    echo "wloc-refresh-set: no A records resolved (DNS unavailable?)" >&2
    exit 1
}

nft add table inet "$TABLE" 2>/dev/null || true
nft flush set inet "$TABLE" "$SET" 2>/dev/null || nft add set inet "$TABLE" "$SET" '{ type ipv4_addr; }'
nft flush set inet "$TABLE" apple_hosts6 2>/dev/null || nft add set inet "$TABLE" apple_hosts6 '{ type ipv6_addr; }'
nft add element inet "$TABLE" "$SET" "{ $ips4 }"
# Global (non link-local, non ULA) AAAA answers only: the guard is about
# internet-side Apple endpoints, never about on-link addresses.
ips6=$(collect_aaaa | grep -vE '^(fe[89ab]|f[cd][0-9a-f]{2}):' | sort -u | csv)
if [ -n "$ips6" ]; then
    nft add element inet "$TABLE" apple_hosts6 "{ $ips6 }"
fi
mkdir -p /var/run/wloc-service
# Local DNS maps each approved name to this router for stable ingress. Keep a
# separate public answer for each name: an Apple/Autonavi CDN address is not
# interchangeable across TLS hostnames.
upstream_map=/var/run/wloc-service/upstream-map
# $$ suffix: init start, a daemon-triggered sync, and a manual run can
# overlap; concurrent writers to a shared tmp would mv a corrupted map
# into place. (The stop path also removes the legacy unsuffixed name.)
upstream_map_tmp=$upstream_map.tmp.$$
: > "$upstream_map_tmp"
for host in $HOSTS; do
    resolve_a "$host" 2>/dev/null \
        | head -n 4 \
        | sed "s/^/$host /" >> "$upstream_map_tmp" || true
done
[ -s "$upstream_map_tmp" ] || {
    rm -f "$upstream_map_tmp"
    echo "wloc-refresh-set: no host-specific public A records resolved" >&2
    exit 1
}
mv "$upstream_map_tmp" "$upstream_map"
echo "wloc-refresh-set: updated $SET = { $ips4 }"
