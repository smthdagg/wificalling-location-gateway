#!/bin/sh
set -eu

nodes=${1:?node list required}
# compact_status_marker: static export for the LuCI view (the
# /ubus JSON-RPC channel truncates larger replies on some firmwares).
output=${2:-/www/wloc-node-status.json}
target=${3:-}
tmp="${output}.tmp.$$"
lock=/tmp/node-health.lock
# ponytail: serialize one status sweep; split by node only if measured fleets
# need sub-minute probes.
if ! mkdir "$lock" 2>/dev/null; then
	lock_pid=$(cat "$lock/pid" 2>/dev/null || echo 0)
	if kill -0 "$lock_pid" 2>/dev/null; then
		exit 0
	fi
	rm -f "$lock/pid"; rmdir "$lock" 2>/dev/null || true
	mkdir "$lock" 2>/dev/null || exit 0
fi
echo $$ > "$lock/pid"
trap 'rm -f "$tmp" "$lock/pid"; rmdir "$lock" 2>/dev/null || true' EXIT HUP INT TERM

json_escape() {
	printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

# This is endpoint quality, not a proxy handshake. It must remain independent
# of sing-box so status polling cannot increase Gateway memory pressure.
node_icmp_test() {
	id=$1; server=$2; refresh=${3:-1}
	cache="/tmp/node-health-$id"
	if [ -f "$cache" ]; then
		cache_ts=$(sed -n '1p' "$cache" 2>/dev/null || echo 0)
		case "$cache_ts" in *[!0-9]*|'') cache_ts=0;; esac
		age=$(($(date +%s) - cache_ts))
		if { [ "$age" -ge 0 ] && [ "$age" -lt 60 ]; } || [ "$refresh" -eq 0 ]; then
			[ "$(sed -n '2p' "$cache")" = ok ] || return 1
			printf '%s' "$(sed -n '3p' "$cache")"
			return 0
		fi
	fi
	[ "$refresh" -eq 1 ] || return 2
	result=$(ping -q -c 1 -W 2 "$server" 2>/dev/null || true)
	ping_ms=$(printf '%s\n' "$result" | sed -n 's/.*= [0-9.]*\/\([0-9.]*\)\/[0-9.]* ms.*/\1/p')
	if [ -n "$ping_ms" ]; then
		printf '%s\nok\n%s\n' "$(date +%s)" "$ping_ms" > "$cache"
		printf '%s' "$ping_ms"
		return 0
	fi
	printf '%s\nfailed\nicmp_failed\n' "$(date +%s)" > "$cache"
	return 1
}

{
	printf '{"generated_at":%s,"nodes":[' "$(date +%s)"
	first=1
	while IFS='|' read -r id label protocol server port ignored_probe_port; do
		[ -n "$id" ] || continue
		refresh=0
		[ -z "$target" ] || [ "$id" = "$target" ] && refresh=1
		state=unreachable; measurement=icmp; ping_json=null; reason_json='"icmp_failed"'
		if ping_ms=$(node_icmp_test "$id" "$server" "$refresh"); then
			state=reachable; ping_json=$ping_ms; reason_json=null
		elif [ "$?" -eq 2 ]; then
			state=unknown; reason_json='"not_yet_checked"'
		fi
		[ "$first" -eq 1 ] || printf ','
		first=0
		printf '{"id":"%s","state":"%s","measurement":"%s","ping_ms":%s,"reason":%s}' \
			"$(json_escape "$id")" "$state" "$measurement" "$ping_json" "$reason_json"
	done < "$nodes"
	printf ']}\n'
} > "$tmp"
chmod 644 "$tmp"
mv "$tmp" "$output"
rm -f "$lock/pid"; rmdir "$lock" 2>/dev/null || true
trap - EXIT HUP INT TERM
