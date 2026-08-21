#!/bin/sh
# Measure one bounded command without recording its stdout/stderr.
set -eu

report=${WLOC_RESOURCE_REPORT:-}
if [ "$#" -gt 0 ] && [ "$1" = --report ]; then
	[ "$#" -ge 2 ] || { echo 'profile-resource: --report needs a path' >&2; exit 2; }
	report=$2
	shift 2
fi
[ -n "$report" ] || { echo 'profile-resource: WLOC_RESOURCE_REPORT is required' >&2; exit 2; }
if [ "$#" -lt 2 ] || [ "$1" != -- ]; then
	echo 'usage: profile-resource.sh [--report PATH] -- COMMAND [ARGS...]' >&2
	exit 2
fi
shift
[ "$#" -gt 0 ] || { echo 'profile-resource: command is required' >&2; exit 2; }

parent=${report%/*}
[ "$parent" = "$report" ] && parent=.
[ -d "$parent" ] || { echo 'profile-resource: report parent is missing' >&2; exit 2; }
[ ! -L "$report" ] || { echo 'profile-resource: report must not be a symlink' >&2; exit 2; }

tmp=$(mktemp "${TMPDIR:-/tmp}/wloc-resource-time.XXXXXX")
trap 'rm -f "$tmp"' EXIT HUP INT TERM
time_bin=
for candidate in /usr/bin/time "$(command -v gtime 2>/dev/null || true)"; do
	if [ -x "$candidate" ] && "$candidate" -f '%e' true >/dev/null 2>/dev/null; then
		time_bin=$candidate
		break
	fi
done
if [ -z "$time_bin" ]; then
	python3=$(command -v python3 || true)
	[ -n "$python3" ] || { echo 'profile-resource: GNU time or Python 3 is required' >&2; exit 127; }
	exec "$python3" "$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)/profile-resource.py" \
		--report "$report" -- "$@"
fi
set +e
"$time_bin" -f 'elapsed_seconds=%e\npeak_rss_kib=%M\ncpu_percent_raw=%P\n' \
	"$@" >/dev/null 2>"$tmp"
command_status=$?
set -e

elapsed_seconds=$(sed -n 's/^elapsed_seconds=//p' "$tmp")
peak_rss_kib=$(sed -n 's/^peak_rss_kib=//p' "$tmp")
cpu_percent_raw=$(sed -n 's/^cpu_percent_raw=//p' "$tmp" | tr -d '%')
case "$elapsed_seconds:$peak_rss_kib:$cpu_percent_raw" in
	''|*[!0-9.:%-]*) echo 'profile-resource: malformed time output' >&2; exit 1 ;;
esac
elapsed_ms=$(awk -v value="$elapsed_seconds" 'BEGIN { printf "%d", (value * 1000) + 0.5 }')
cpu_percent=$(awk -v value="$cpu_percent_raw" 'BEGIN { printf "%d", value + 0.5 }')

if [ "$command_status" -eq 0 ]; then status=pass; else status=fail; fi
umask 077
{
	printf 'status=%s\n' "$status"
	printf 'elapsed_ms=%s\n' "$elapsed_ms"
	printf 'peak_rss_kib=%s\n' "$peak_rss_kib"
	printf 'cpu_percent=%s\n' "$cpu_percent"
	printf 'command_status=%s\n' "$command_status"
} > "$report"
[ "$command_status" -eq 0 ] || exit "$command_status"
