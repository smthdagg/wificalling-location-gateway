#!/bin/sh
# Emit a bounded, coordinate/device-redacted status list for v2 profiles.
#
# This is intentionally a read-only health projection. It does not decide
# lifecycle state or modify UCI/nftables; the runtime manager remains the
# authority for enable/disable ordering.

set -eu

MAX_PROFILES=8
NFT_BINARY=${WLOC_NFT_BINARY:-nft}

json_escape() {
	printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/[[:space:]]\+/ /g'
}

valid_profile_id() {
	case "$1" in
		''|*[!a-z0-9_-]*) return 1 ;;
	esac
	[ "${#1}" -le 32 ]
}

uci_get() {
	uci -q get "$1" 2>/dev/null || true
}

profile_phase() {
	profile_id=$1
	enabled=$2
	assigned_device=$3
	table="wloc_profile_${profile_id}"
	[ "$profile_id" = default ] && table=wloc_service
	if [ "$enabled" != 1 ]; then
		printf '%s' 'disabled|disabled'
	elif [ -z "$assigned_device" ] && [ "$profile_id" != default ]; then
		printf '%s' 'degraded_passthrough|missing_device_binding'
	elif "$NFT_BINARY" list table inet "$table" >/dev/null 2>&1; then
		printf '%s' 'intercepting|intercepting'
	else
		printf '%s' 'passthrough|redirect_not_installed'
	fi
}

sections=$(uci -q show wloc-service 2>/dev/null \
	| sed -n 's/^wloc-service\.\([a-z0-9_-]*\)=device$/\1/p' \
	| head -n "$MAX_PROFILES" || true)
if [ -z "$sections" ]; then
	# Keep the v1 singleton visible during migration without returning its
	# address. It is not treated as a v2 device section by the parser.
	if [ -n "$(uci_get wloc-service.main.enabled)" ]; then
		sections=default
	fi
fi

printf '{"profiles":['
first=1
for profile_id in $sections; do
	valid_profile_id "$profile_id" || continue
	if [ "$profile_id" = default ]; then
		prefix=wloc-service.main
		label='Default device'
	else
		prefix="wloc-service.$profile_id"
		label=$(uci_get "$prefix.label" | cut -c1-48)
		[ -n "$label" ] || label=$profile_id
	fi
	enabled=$(uci_get "$prefix.enabled")
	[ "$enabled" = 1 ] || enabled=0
	assigned_device=$(uci_get "$prefix.assigned_device")
	phase_reason=$(profile_phase "$profile_id" "$enabled" "$assigned_device")
	phase=${phase_reason%%|*}
	reason=${phase_reason#*|}
	[ "$first" -eq 1 ] || printf ','
	first=0
	printf '{"id":"%s","label":"%s","enabled":%s,"assigned_device_configured":%s,"phase":"%s","reason_code":"%s"}' \
		"$(json_escape "$profile_id")" "$(json_escape "$label")" "$([ "$enabled" = 1 ] && printf true || printf false)" \
		"$([ -n "$assigned_device" ] && printf true || printf false)" "$phase" "$reason"
done
printf ']}\n'
