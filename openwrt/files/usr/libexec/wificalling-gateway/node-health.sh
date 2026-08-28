#!/bin/sh
set -eu

nodes=${1:?node list required}
# compact_status_marker: static export for the LuCI view (the
# /ubus JSON-RPC channel truncates larger replies on some firmwares).
output=${2:-/www/wloc-node-status.json}
tmp="${output}.tmp.$$"
trap 'rm -f "$tmp"' EXIT HUP INT TERM

json_escape() {
	printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

probe_port_for() {
	local id hash
	hash=$(printf '%s' "$1" | md5sum | cut -c1-4)
	printf '%d' "$((20000 + (0x$hash % 10000)))"
}

# Probe through the loopback inbound compiled into the already-running
# Gateway sing-box. Each node's inbound is routed to that node's outbound,
# which validates the complete proxy path without creating another process.
# Cache entries last 60s; the cache is only a status optimization and never
# starts or owns a network process.
node_proxy_test() {
	id=$1; probe_port=$2
	cache="/tmp/node-health-$id"
	lock="/tmp/node-health.lock"
	if [ -f "$cache" ]; then
		cache_ts=$(sed -n '1p' "$cache" 2>/dev/null || echo 0)
		case "$cache_ts" in *[!0-9]*|'') cache_ts=0;; esac
		age=$(($(date +%s) - cache_ts))
		if [ "$age" -ge 0 ] && [ "$age" -lt 60 ]; then
			[ "$(sed -n '2p' "$cache")" = ok ] || return 1
			printf '%s|%s' "$(sed -n '3p' "$cache")" "$(sed -n '4p' "$cache")"
			return 0
		fi
	fi
	if ! mkdir "$lock" 2>/dev/null; then
		lock_pid=$(cat "$lock/pid" 2>/dev/null || echo 0)
		if ! kill -0 "$lock_pid" 2>/dev/null; then
			rm -f "$lock/pid"; rmdir "$lock" 2>/dev/null || true
			mkdir "$lock" 2>/dev/null || true
		fi
	fi
	if ! [ -d "$lock" ]; then
		return 1
	fi
	echo $$ > "$lock/pid"
	result=$(curl -sS --max-time 8 -x "http://127.0.0.1:$probe_port" \
		-w '\n%{time_total}' 'http://ip-api.com/json?fields=query' 2>/dev/null || true)
	seconds=$(printf '%s\n' "$result" | tail -n 1)
	ping_ms=$(printf '%s\n' "$seconds" | awk '/^[0-9]+(\.[0-9]+)?$/ { printf "%.2f", $1 * 1000 }')
	body=$(printf '%s\n' "$result" | sed '$d')
	exit_ip=$(printf '%s\n' "$body" | sed -n 's/.*"query":"\([0-9.]*\)".*/\1/p')
	if [ -n "$ping_ms" ]; then
		printf '%s\nok\n%s\n%s\n' "$(date +%s)" "$ping_ms" "$exit_ip" > "$cache"
		rm -f "$lock/pid"; rmdir "$lock" 2>/dev/null || true
		printf '%s|%s' "$ping_ms" "$exit_ip"
		return 0
	fi
	rm -f "$lock/pid"; rmdir "$lock" 2>/dev/null || true
	printf '%s\nfailed\nproxy_failed\n' "$(date +%s)" > "$cache"
	return 1
}

{
	printf '{"generated_at":%s,"nodes":[' "$(date +%s)"
	first=1
	while IFS='|' read -r id label protocol server port probe_port; do
		[ -n "$id" ] || continue
		[ -n "$probe_port" ] || probe_port=$(probe_port_for "$id")
		state=proxy_failed; measurement=proxy; ping_json=null; reason_json='"proxy_failed"'
		if result=$(node_proxy_test "$id" "$probe_port"); then
			ping_ms=${result%%|*}
			state=proxy_reachable; ping_json=$ping_ms; reason_json=null
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
trap - EXIT HUP INT TERM
