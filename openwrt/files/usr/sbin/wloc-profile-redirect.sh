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
NAMES='gs-loc.apple.com gs-loc-cn.apple.com'

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
		10|192) return 0 ;;
		172) [ "$2" -ge 16 ] 2>/dev/null && [ "$2" -le 31 ] 2>/dev/null ;;
		*) return 1 ;;
	esac
}

action=${1:-}
profile_id=${2:-}
nft_binary=${WLOC_NFT_BINARY:-nft}

case "$action" in
	start)
		[ "$#" -eq 3 ] || fail 'usage: start PROFILE_ID PRIVATE_IPV4'
		valid_profile_id "$profile_id" || fail 'invalid profile id'
		valid_private_ipv4 "$3" || fail 'assigned device must be a private IPv4 address'
		valid_port "$PROXY_PORT" || fail 'invalid proxy port'
		table=${PROFILE_TABLE_PREFIX}${profile_id}
		device_ip=$3
		"$nft_binary" add table inet "$table" 2>/dev/null || true
		"$nft_binary" add set inet "$table" apple_hosts '{ type ipv4_addr; }' 2>/dev/null || true
		"$nft_binary" flush chain inet "$table" prerouting 2>/dev/null || true
		"$nft_binary" delete chain inet "$table" prerouting 2>/dev/null || true
		"$nft_binary" "add chain inet $table prerouting { type filter hook prerouting priority mangle; }"
		"$nft_binary" "add rule inet $table prerouting ip saddr $device_ip tcp dport 443 ip daddr @apple_hosts meta mark set $FWMARK tproxy ip to :$PROXY_PORT"
		printf 'wloc-profile-redirect: %s -> :%s\n' "$profile_id" "$PROXY_PORT"
		;;
	stop)
		[ "$#" -eq 2 ] || fail 'usage: stop PROFILE_ID'
		valid_profile_id "$profile_id" || fail 'invalid profile id'
		"$nft_binary" delete table inet "${PROFILE_TABLE_PREFIX}${profile_id}" 2>/dev/null || true
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
