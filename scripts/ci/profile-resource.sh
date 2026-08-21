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

timeout_seconds=${WLOC_RESOURCE_TIMEOUT_SECONDS:-30}
case "$timeout_seconds" in ''|*[!0-9]*) echo 'profile-resource: timeout must be a positive integer' >&2; exit 2 ;; esac
[ "$timeout_seconds" -gt 0 ] || { echo 'profile-resource: timeout must be a positive integer' >&2; exit 2; }
time_bin=
if [ "${WLOC_RESOURCE_FORCE_PROCFS:-0}" != 1 ]; then
	for candidate in /usr/bin/time "$(command -v gtime 2>/dev/null || true)"; do
		if [ -x "$candidate" ] && "$candidate" -f '%e' true >/dev/null 2>/dev/null; then
			time_bin=$candidate
			break
		fi
	done
fi
timeout_bin=$(command -v timeout || true)
if [ -n "$time_bin" ] && [ -z "$timeout_bin" ] \
	&& [ -r /proc/self/status ] && [ -r /proc/self/stat ]; then
	time_bin=
fi
if [ -z "$time_bin" ]; then
	python3=
	if [ "${WLOC_RESOURCE_FORCE_PROCFS:-0}" != 1 ]; then
		python3=$(command -v python3 || true)
	fi
	if [ -n "$python3" ]; then
		exec "$python3" "$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)/profile-resource.py" \
			--timeout-seconds "$timeout_seconds" --report "$report" -- "$@"
	fi
	[ -r /proc/self/status ] && [ -r /proc/self/stat ] || {
		echo 'profile-resource: GNU time, Python 3, or procfs is required' >&2
		exit 127
	}

	proc_ticks() {
		awk '{ print $14 + $15 }' "/proc/$1/stat" 2>/dev/null || printf '0\n'
	}
	proc_rss_kib() {
		awk '/^VmRSS:/ { print $2; exit }' "/proc/$1/status" 2>/dev/null || printf '0\n'
	}

	set +e
	"$@" >/dev/null 2>&1 &
	pid=$!
	started=$(date +%s)
	start_ticks=$(proc_ticks "$pid")
	last_ticks=$start_ticks
	peak_rss_kib=$(proc_rss_kib "$pid")
	case "$peak_rss_kib" in ''|*[!0-9]*) peak_rss_kib=0 ;; esac
	timed_out=0
	while kill -0 "$pid" 2>/dev/null; do
		now=$(date +%s)
		if [ $((now - started)) -ge "$timeout_seconds" ]; then
			kill "$pid" 2>/dev/null || true
			sleep 1
			kill -9 "$pid" 2>/dev/null || true
			timed_out=1
		fi
		rss=$(proc_rss_kib "$pid")
		case "$rss" in ''|*[!0-9]*) rss=0 ;; esac
		[ "$rss" -gt "$peak_rss_kib" ] && peak_rss_kib=$rss
		last_ticks=$(proc_ticks "$pid")
		sleep 0.1
	done
	wait "$pid"
	command_status=$?
	ended=$(date +%s)
	set -e

	elapsed_seconds=$((ended - started))
	[ "$elapsed_seconds" -gt 0 ] || elapsed_seconds=1
	hz=$(getconf CLK_TCK 2>/dev/null || printf '100\n')
	case "$hz" in ''|*[!0-9]*) hz=100 ;; esac
	delta_ticks=$((last_ticks - start_ticks))
	[ "$delta_ticks" -gt 0 ] || delta_ticks=0
	cpu_percent=$((delta_ticks * 100 / (elapsed_seconds * hz)))
	[ "$peak_rss_kib" -gt 0 ] || command_status=125
	[ "$timed_out" -eq 0 ] || command_status=124
	if [ "$command_status" -eq 0 ]; then status=pass; else status=fail; fi
	umask 077
	{
		printf 'status=%s\n' "$status"
		printf 'elapsed_ms=%s\n' "$((elapsed_seconds * 1000))"
		printf 'peak_rss_kib=%s\n' "$peak_rss_kib"
		printf 'cpu_percent=%s\n' "$cpu_percent"
		printf 'command_status=%s\n' "$command_status"
	} > "$report"
	exit "$command_status"
fi

[ -n "$timeout_bin" ] || {
	echo 'profile-resource: timeout utility or procfs is required for GNU time mode' >&2
	exit 127
}
tmp=$(mktemp "${TMPDIR:-/tmp}/wloc-resource-time.XXXXXX")
trap 'rm -f "$tmp"' EXIT HUP INT TERM
set +e
"$time_bin" -f 'elapsed_seconds=%e\npeak_rss_kib=%M\ncpu_percent_raw=%P\n' \
	"$timeout_bin" "$timeout_seconds" "$@" >/dev/null 2>"$tmp"
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
