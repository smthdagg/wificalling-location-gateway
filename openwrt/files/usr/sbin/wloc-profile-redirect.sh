#!/bin/sh
# Manage one device profile's WLOC TPROXY table.
#
# The table is intentionally profile-scoped. A profile stop deletes only its
# own table; the shared policy route and the stable Gateway nftables namespace
# are owned elsewhere. The only approved destinations are the two exact Apple
# WLOC hostnames, represented by the per-table apple_hosts set populated by
# wloc-refresh-set.sh.

set -eu

PROFILE_TABLE_PREFIX=wloc_profile_
PROXY_PORT=${WLOC_PROFILE_PROXY_PORT:-${WLOC_PROXY_PORT:-8443}}
FWMARK=1
ROUTE_TABLE=100
IP_BINARY=${WLOC_IP_BINARY:-ip}

fail() {
	printf 'wloc-profile-redirect: %s\n' "$*" >&2
	exit 2
}

valid_profile_id() {
	case "$1" in
		''|*[!a-z0-9_-]*) return 1 ;;
	esac
	[ "${#1}" -le 32 ]
}

valid_port() {
	case "$1" in
		''|*[!0-9]*) return 1 ;;
	esac
	[ "$1" -ge 1 ] 2>/dev/null && [ "$1" -le 65535 ] 2>/dev/null
}

valid_private_ipv4() {
	address=$1
	old_ifs=$IFS
	IFS=.
	set -- $address
	IFS=$old_ifs
	[ "$#" -eq 4 ] || return 1
	for octet in "$@"; do
		case "$octet" in
			''|*[!0-9]*) return 1 ;;
		esac
		[ "$octet" -le 255 ] 2>/dev/null || return 1
	done
	case "$1" in
		10) return 0 ;;
		192) [ "$2" -eq 168 ] 2>/dev/null ;;
		172) [ "$2" -ge 16 ] 2>/dev/null && [ "$2" -le 31 ] 2>/dev/null ;;
		*) return 1 ;;
	esac
}

stop_all_profiles() {
	# This is the crash/upgrade cleanup boundary. Only tables with the exact
	# component-owned prefix are eligible; the stable Gateway namespace is
	# never enumerated or modified here.
	for table in $("$nft_binary" list tables inet 2>/dev/null \
		| sed -n 's/^table inet \(wloc_profile_[a-z0-9_-]*\)$/\1/p'); do
		case "$table" in
			wloc_profile_*|*[!a-z0-9_-]*)
				case "$table" in *[!a-z0-9_-]*) continue ;; esac
				;;
			*) continue ;;
		esac
		"$nft_binary" delete table inet "$table" 2>/dev/null || true
	done
	remove_policy_route
}

install_policy_route() {
	"$IP_BINARY" rule del fwmark "$FWMARK" lookup "$ROUTE_TABLE" 2>/dev/null || true
	"$IP_BINARY" route del local 0.0.0.0/0 dev lo table "$ROUTE_TABLE" 2>/dev/null || true
	"$IP_BINARY" rule add fwmark "$FWMARK" lookup "$ROUTE_TABLE"
	"$IP_BINARY" route add local 0.0.0.0/0 dev lo table "$ROUTE_TABLE"
}

remove_policy_route() {
	"$IP_BINARY" rule del fwmark "$FWMARK" lookup "$ROUTE_TABLE" 2>/dev/null || true
	"$IP_BINARY" route del local 0.0.0.0/0 dev lo table "$ROUTE_TABLE" 2>/dev/null || true
}

action=${1:-}
profile_id=${2:-}
nft_binary=${WLOC_NFT_BINARY:-nft}

case "$action" in
	route-start)
		[ "$#" -eq 1 ] || fail 'usage: route-start'
		install_policy_route
		;;
	route-stop)
		[ "$#" -eq 1 ] || fail 'usage: route-stop'
		remove_policy_route
		;;
	stop-all)
		[ "$#" -eq 1 ] || fail 'usage: stop-all'
		stop_all_profiles
		;;
	start)
		[ "$#" -eq 3 ] || fail 'usage: start PROFILE_ID PRIVATE_IPV4'
		valid_profile_id "$profile_id" || fail 'invalid profile id'
		valid_private_ipv4 "$3" || fail 'assigned device must be a private IPv4 address'
		valid_port "$PROXY_PORT" || fail 'invalid proxy port'
		table=${PROFILE_TABLE_PREFIX}${profile_id}
		device_ip=$3
		"$nft_binary" add table inet "$table" 2>/dev/null || true
		"$nft_binary" add set inet "$table" apple_hosts '{ type ipv4_addr; flags timeout; timeout 30s; }' 2>/dev/null || true
		"$nft_binary" flush chain inet "$table" prerouting 2>/dev/null || true
		"$nft_binary" delete chain inet "$table" prerouting 2>/dev/null || true
		"$nft_binary" "add chain inet $table prerouting { type filter hook prerouting priority mangle; }"
		"$nft_binary" "add rule inet $table prerouting ip saddr $device_ip tcp dport 443 ip daddr @apple_hosts meta mark set $FWMARK tproxy ip to :$PROXY_PORT"
		install_policy_route
		printf 'wloc-profile-redirect: %s -> :%s\n' "$profile_id" "$PROXY_PORT"
		;;
	stop)
		[ "$#" -eq 2 ] || fail 'usage: stop PROFILE_ID'
		valid_profile_id "$profile_id" || fail 'invalid profile id'
		"$nft_binary" delete table inet "${PROFILE_TABLE_PREFIX}${profile_id}" 2>/dev/null || true
		remaining=$("$nft_binary" list tables inet 2>/dev/null \
			| sed -n 's/^table inet \(wloc_profile_[a-z0-9_-]*\)$/\1/p')
		[ -n "$remaining" ] || remove_policy_route
		;;
	status)
		[ "$#" -eq 2 ] || fail 'usage: status PROFILE_ID'
		valid_profile_id "$profile_id" || fail 'invalid profile id'
		"$nft_binary" list table inet "${PROFILE_TABLE_PREFIX}${profile_id}" >/dev/null 2>&1
		;;
	*)
		fail 'action must be start, stop, or status'
		;;
esac
