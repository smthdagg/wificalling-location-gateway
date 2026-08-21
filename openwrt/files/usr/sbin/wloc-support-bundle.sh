#!/bin/sh
set -eu

# Create a small, privacy-safe diagnostics archive. This deliberately collects
# health summaries and event codes rather than UCI, status JSON, packet data,
# node material, device addresses, or coordinates.
OUTPUT=${WLOC_SUPPORT_OUTPUT:-/tmp/wloc-support-bundle.tar.gz}
HEALTH=${WLOC_SUPPORT_HEALTH:-/var/run/wloc-service/health.json}
WLOC_LOG=${WLOC_SUPPORT_WLOC_LOG:-/var/run/wloc-service/events.jsonl}
PROFILE_LOG_ROOT=${WLOC_SUPPORT_PROFILE_LOG_ROOT:-/var/run/wloc-service/profiles}
MAX_BYTES=${WLOC_SUPPORT_MAX_BYTES:-65536}
LOCK=/tmp/wloc-support-bundle.lock

case "$MAX_BYTES" in
	''|*[!0-9]*) MAX_BYTES=65536 ;;
esac
[ "$MAX_BYTES" -ge 4096 ] 2>/dev/null || MAX_BYTES=4096
[ "$MAX_BYTES" -le 131072 ] 2>/dev/null || MAX_BYTES=131072

if ! mkdir "$LOCK" 2>/dev/null; then
	echo '{"error":"support bundle collection already in progress"}' >&2
	exit 1
fi
work=$(mktemp -d /tmp/wloc-support.XXXXXX)
trap 'rm -rf "$work" "$LOCK"' EXIT HUP INT TERM
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
if [ -r "$HEALTH" ]; then
	health_available=true
elif [ -x /usr/sbin/wloc-health.sh ]; then
	if /usr/sbin/wloc-health.sh > "$work/health.generated" 2>/dev/null; then
		health_available=true
	fi
fi
wloc_available=false
[ -r "$WLOC_LOG" ] && wloc_available=true
profile_logs_available=false

write_events "$WLOC_LOG" "$work/wloc-support/events.jsonl"

# Profile event files are intentionally merged into the same redacted stream.
# The profile directory name is never copied to the archive; only bounded,
# stable event envelope fields survive write_events(). At most eight profiles
# are inspected, matching the validated runtime profile limit.
if [ -d "$PROFILE_LOG_ROOT" ] && [ ! -L "$PROFILE_LOG_ROOT" ]; then
	for profile_dir in "$PROFILE_LOG_ROOT"/*; do
		[ -d "$profile_dir" ] || continue
		[ ! -L "$profile_dir" ] || continue
		profile_id=${profile_dir##*/}
		case "$profile_id" in
			''|*[!a-z0-9_-]*) continue ;;
		esac
		profile_log="$profile_dir/events.jsonl"
		[ -r "$profile_log" ] || continue
		profile_logs_available=true
		write_events "$profile_log" "$work/profile-events.jsonl"
		cat "$work/profile-events.jsonl" >> "$work/wloc-support/events.jsonl"
	done
fi

if [ "$profile_logs_available" = true ]; then
	# Keep the aggregate within the same event budget after profile appends.
	tail -c 16384 "$work/wloc-support/events.jsonl" > "$work/wloc-support/events.jsonl.tmp"
	mv "$work/wloc-support/events.jsonl.tmp" "$work/wloc-support/events.jsonl"
fi

cat > "$work/wloc-support/manifest.txt" <<EOF
schema=wificalling-location-gateway.support.v1
privacy=redacted-no-credentials-no-device-identifiers-no-precise-location
health_available=$health_available
wloc_log_available=$wloc_available
profile_logs_available=$profile_logs_available
EOF

# The raw health document is intentionally not copied: it can contain profile
# labels, IDs, addresses, and effective coordinates. These booleans are enough
# to tell whether collection reached the local service.
printf '{"schema":"wificalling-location-gateway.support.v1","health_available":%s,"wloc_log_available":%s,"profile_logs_available":%s}\n' \
	"$health_available" "$wloc_available" "$profile_logs_available" > "$work/wloc-support/health.json"

output_dir=${OUTPUT%/*}
[ "$output_dir" = "$OUTPUT" ] && output_dir=.
[ -d "$output_dir" ] || { echo '{"error":"support bundle output directory is unavailable"}' >&2; exit 1; }
[ ! -L "$OUTPUT" ] || { echo '{"error":"support bundle output must not be a symlink"}' >&2; exit 1; }
archive="$work/wloc-support-bundle.tar.gz"
if ! tar -czf "$archive" -C "$work" wloc-support; then
	rm -f "$archive"
	echo '{"error":"support bundle archive creation failed"}' >&2
	exit 1
fi
size=$(wc -c < "$archive" | tr -d ' ')
if [ "$size" -gt "$MAX_BYTES" ]; then
	# Preserve the manifest and health summary under pressure; never emit an
	# over-cap bundle or silently truncate a tar stream.
	rm -f "$work/wloc-support/events.jsonl"
	rm -f "$archive"
	if ! tar -czf "$archive" -C "$work" wloc-support; then
		rm -f "$archive"
		echo '{"error":"support bundle archive creation failed"}' >&2
		exit 1
	fi
	size=$(wc -c < "$archive" | tr -d ' ')
fi
[ "$size" -le "$MAX_BYTES" ] || { rm -f "$archive"; echo '{"error":"support bundle exceeds storage cap"}' >&2; exit 1; }
chmod 600 "$archive"
[ ! -L "$OUTPUT" ] || { rm -f "$archive"; echo '{"error":"support bundle output became a symlink"}' >&2; exit 1; }
mv -f "$archive" "$OUTPUT"
chmod 600 "$OUTPUT"
printf '{"path":"%s","bytes":%s,"expires_seconds":600}\n' "$OUTPUT" "$size"
