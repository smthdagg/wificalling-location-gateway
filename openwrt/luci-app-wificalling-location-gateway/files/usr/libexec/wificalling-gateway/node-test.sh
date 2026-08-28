#!/bin/sh
# node-test.sh — manual endpoint latency test for one proxy node.
# Prints one JSON object; always exits 0 so rpcd forwards the reply untouched.

set -eu

id=${1:?node id required}

server=$(uci -q get "wificalling-gateway.$id.server") || true
port=$(uci -q get "wificalling-gateway.$id.port") || true
if [ -z "$server" ] || [ -z "$port" ]; then
	printf '{"state":"failed","reason":"config_missing"}\n'
	exit 0
fi

result=$(ping -q -c 1 -W 2 "$server" 2>/dev/null || true)
ms=$(printf '%s\n' "$result" | sed -n 's/.*= [0-9.]*\/\([0-9.]*\)\/[0-9.]* ms.*/\1/p')
[ -n "$ms" ] || {
	printf '{"state":"unreachable","reason":"icmp_failed"}\n'
	exit 0
}
printf '{"state":"reachable","measurement":"icmp","ping_ms":"%s"}\n' "$ms"
exit 0
