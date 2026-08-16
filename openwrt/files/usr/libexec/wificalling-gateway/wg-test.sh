#!/bin/sh
# wg-test.sh — manual WireGuard handshake test for one node.
#
# Runs the same handshake probe the monitor loop uses (the function is
# extracted from the patched node-health.sh, so there is exactly one
# implementation), but bypasses the 60s result cache so the user gets a
# fresh answer on demand. Prints one JSON object; always exits 0 so rpcd
# forwards the reply untouched.

set -eu

id=${1:?node id required}

proto=$(uci -q get "wificalling-gateway.$id.protocol") || true
if [ "$proto" != wireguard ]; then
	printf '{"state":"skipped","reason":"not_wireguard"}\n'
	exit 0
fi

server=$(uci -q get "wificalling-gateway.$id.server") || true
port=$(uci -q get "wificalling-gateway.$id.port") || true
if [ -z "$server" ] || [ -z "$port" ]; then
	printf '{"state":"failed","reason":"config_missing"}\n'
	exit 0
fi

health=/usr/libexec/wificalling-gateway/node-health.sh
[ -f "$health" ] || {
	printf '{"state":"failed","reason":"no_health_script"}\n'
	exit 0
}

# Extract the handshake function from the patched monitor script so the
# manual test and the monitor loop share one implementation.
func=$(mktemp /tmp/wg-test-func.XXXXXX)
trap 'rm -f "$func"' EXIT HUP INT TERM
awk '/^wg_handshake_test\(\)/,/^}/' "$health" > "$func"
. "$func"

# The monitor loop may be mid-test right now; wait for its lock so this
# run is authoritative (a handshake takes up to ~8s, give it 20s).
n=0
while [ -d /tmp/wg-health.lock ]; do
	n=$((n + 1))
	[ "$n" -ge 40 ] && {
		printf '{"state":"failed","reason":"busy"}\n'
		exit 0
	}
	sleep 1
done

# Bypass the 60s cache: the cached result is exactly what the user is
# asking to re-check.
rm -f "/tmp/wg-health-$id"

if exit_ip=$(wg_handshake_test "$id" "$server" "$port"); then
	printf '{"state":"handshake_ok","exit_ip":"%s"}\n' "$exit_ip"
else
	reason=$(sed -n '3p' "/tmp/wg-health-$id" 2>/dev/null || echo unreachable)
	printf '{"state":"handshake_failed","reason":"%s"}\n' "$reason"
fi
exit 0
