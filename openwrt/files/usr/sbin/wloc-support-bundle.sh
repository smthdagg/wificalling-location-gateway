#!/bin/sh
set -eu

# Create a small, privacy-safe diagnostics archive. This deliberately collects
# health summaries and event codes rather than UCI, status JSON, packet data,
# node material, device addresses, or coordinates.
OUTPUT=${WLOC_SUPPORT_OUTPUT:-/tmp/wloc-support-bundle.tar.gz}
HEALTH=${WLOC_SUPPORT_HEALTH:-/var/run/wloc-service/health.json}
WLOC_LOG=${WLOC_SUPPORT_WLOC_LOG:-/var/run/wloc-service/events.jsonl}
GATEWAY_LOG=${WLOC_SUPPORT_GATEWAY_LOG:-/var/run/wificalling-gateway/events.log}
MAX_BYTES=${WLOC_SUPPORT_MAX_BYTES:-65536}

case "$MAX_BYTES" in
	''|*[!0-9]*) MAX_BYTES=65536 ;;
esac
[ "$MAX_BYTES" -ge 4096 ] 2>/dev/null || MAX_BYTES=4096
[ "$MAX_BYTES" -le 131072 ] 2>/dev/null || MAX_BYTES=131072

work=$(mktemp -d /tmp/wloc-support.XXXXXX)
trap 'rm -rf "$work"' EXIT HUP INT TERM
mkdir -p "$work/wloc-support"

safe_token() {
	printf '%s' "$1" | tr -cd 'A-Za-z0-9_.-' | cut -c1-64
}

write_events() {
	input=$1
	output=$2
	: > "$output"
	[ -r "$input" ] || return 0
	# Re-emit only stable, non-sensitive event envelope fields. This also
	# drops legacy pipe records instead of guessing whether their label/IP is
	# identifying information.
	while IFS= read -r line; do
		case "$line" in
			\{*\})
				timestamp=$(printf '%s' "$line" | sed -n 's/.*"timestamp"[[:space:]]*:[[:space:]]*\([0-9][0-9]*\).*/\1/p' | cut -c1-16)
				component=$(printf '%s' "$line" | sed -n 's/.*"component"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
				severity=$(printf '%s' "$line" | sed -n 's/.*"severity"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
				event_code=$(printf '%s' "$line" | sed -n 's/.*"event_code"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
				[ -n "$event_code" ] || continue
				[ -n "$timestamp" ] || timestamp=0
				component=$(safe_token "$component"); severity=$(safe_token "$severity"); event_code=$(safe_token "$event_code")
				printf '{"timestamp":%s,"component":"%s","profile_scope":"redacted","severity":"%s","event_code":"%s","message":"redacted diagnostic event"}\n' \
					"$timestamp" "$component" "$severity" "$event_code" >> "$output"
				;;
		esac
	done < "$input"
	tail -c 16384 "$output" > "$output.tmp"
	mv "$output.tmp" "$output"
}

health_available=false
[ -r "$HEALTH" ] && health_available=true
[ -x /usr/sbin/wloc-health.sh ] && health_available=true
gateway_available=false
[ -r "$GATEWAY_LOG" ] && gateway_available=true
wloc_available=false
[ -r "$WLOC_LOG" ] && wloc_available=true

cat > "$work/wloc-support/manifest.txt" <<EOF
schema=wificalling-location-gateway.support.v1
privacy=redacted-no-credentials-no-device-identifiers-no-precise-location
health_available=$health_available
gateway_log_available=$gateway_available
wloc_log_available=$wloc_available
EOF

# The raw health document is intentionally not copied: it can contain profile
# labels, IDs, addresses, and effective coordinates. These booleans are enough
# to tell whether collection reached the local service.
printf '{"schema":"wificalling-location-gateway.support.v1","health_available":%s,"gateway_log_available":%s,"wloc_log_available":%s}\n' \
	"$health_available" "$gateway_available" "$wloc_available" > "$work/wloc-support/health.json"
write_events "$WLOC_LOG" "$work/wloc-support/events.jsonl"
write_events "$GATEWAY_LOG" "$work/wloc-support/gateway-events.jsonl"

tar -czf "$OUTPUT" -C "$work" wloc-support
size=$(wc -c < "$OUTPUT" | tr -d ' ')
if [ "$size" -gt "$MAX_BYTES" ]; then
	# Preserve the manifest and health summary under pressure; never emit an
	# over-cap bundle or silently truncate a tar stream.
	rm -f "$work/wloc-support/events.jsonl" "$work/wloc-support/gateway-events.jsonl"
	tar -czf "$OUTPUT" -C "$work" wloc-support
	size=$(wc -c < "$OUTPUT" | tr -d ' ')
fi
[ "$size" -le "$MAX_BYTES" ] || { rm -f "$OUTPUT"; echo '{"error":"support bundle exceeds storage cap"}' >&2; exit 1; }
chmod 600 "$OUTPUT"
printf '{"path":"%s","bytes":%s,"expires_seconds":600}\n' "$OUTPUT" "$size"
