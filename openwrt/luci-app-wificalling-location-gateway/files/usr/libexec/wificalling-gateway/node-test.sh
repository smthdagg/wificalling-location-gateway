#!/bin/sh
# node-test.sh — manual connection test for one proxy node.
#
# Every protocol is tested through the loopback HTTP inbound of the already
# running Gateway sing-box. The inbound is compiled with a fixed route to
# this node's existing outbound, so no second sing-box process is needed.
# Prints one JSON object; always exits 0 so rpcd forwards the reply untouched.

set -eu

id=${1:?node id required}

server=$(uci -q get "wificalling-gateway.$id.server") || true
port=$(uci -q get "wificalling-gateway.$id.port") || true
if [ -z "$server" ] || [ -z "$port" ]; then
	printf '{"state":"failed","reason":"config_missing"}\n'
	exit 0
fi

test_port=$(printf '%s' "$id" | md5sum | cut -c1-4)
test_port=$((20000 + (0x$test_port % 10000)))
result=$(curl -sS --max-time 8 -x "http://127.0.0.1:$test_port" \
	-w '\n%{time_total}' 'http://ip-api.com/json?fields=query' 2>/dev/null || true)
seconds=$(printf '%s\n' "$result" | tail -n 1)
ms=$(printf '%s\n' "$seconds" | awk '/^[0-9]+(\.[0-9]+)?$/ { printf "%.2f", $1 * 1000 }')
[ -n "$ms" ] || {
	printf '{"state":"unreachable","reason":"proxy_failed"}\n'
	exit 0
}
printf '{"state":"proxy_reachable","measurement":"proxy","ping_ms":"%s"}\n' "$ms"
exit 0
