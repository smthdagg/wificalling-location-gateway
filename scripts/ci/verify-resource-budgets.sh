#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
budget=${WLOC_RESOURCE_BUDGET:-$repo_root/openwrt/files/usr/share/wificalling-location-gateway/resource-budget.conf}
artifact_dir=${WLOC_RESOURCE_ARTIFACT_DIR:-$repo_root/target/release}
package=${WLOC_PACKAGE_ARTIFACT:-}

fail() { echo "verify-resource-budgets: $*" >&2; exit 1; }
value() { sed -n "s/^$1=\([0-9][0-9]*\)$/\1/p" "$budget" | head -n 1; }

if [ ! -f "$budget" ] || [ -L "$budget" ]; then
	fail 'budget file must be a regular file'
fi
[ "$(sed -n 's/^format=//p' "$budget")" = wloc-resource-budget/v1 ] || fail 'unsupported budget format'
for key in \
	runtime_binary_total_max_bytes \
	integrated_package_max_bytes \
	persistent_state_max_bytes \
	log_total_max_bytes \
	cache_total_max_bytes \
	max_profiles \
	startup_max_seconds \
	rss_peak_max_bytes \
	cpu_probe_max_percent; do
	value "$key" | grep -Eq '^[0-9]+$' || fail "invalid or missing budget: $key"
done
binary_limit=$(value runtime_binary_total_max_bytes)
package_limit=$(value integrated_package_max_bytes)
if [ -z "$binary_limit" ] || [ -z "$package_limit" ]; then
	fail 'binary/package limits are required'
fi

total=0
for binary in wloc-gateway-spike wloc-service wloc-ctl; do
	path="$artifact_dir/$binary"
	if [ ! -f "$path" ] || [ -L "$path" ]; then
		fail "missing regular runtime binary: $path"
	fi
	size=$(wc -c < "$path" | tr -d ' ')
	case "$size" in ''|*[!0-9]*) fail "invalid size for $binary" ;; esac
	total=$((total + size))
done
[ "$total" -le "$binary_limit" ] || fail "runtime binaries exceed ${binary_limit} bytes: $total"

if [ -n "$package" ]; then
	if [ ! -f "$package" ] || [ -L "$package" ]; then
		fail 'package artifact must be a regular file'
	fi
	package_size=$(wc -c < "$package" | tr -d ' ')
	[ "$package_size" -le "$package_limit" ] || fail "integrated package exceeds ${package_limit} bytes: $package_size"
fi

report=${WLOC_RESOURCE_REPORT:-}
if [ -n "$report" ]; then
	[ -f "$report" ] || fail "resource report is missing: $report"
	grep -Fx 'status=pass' "$report" >/dev/null || fail 'resource report records a failed command'
	grep -Fx 'command_status=0' "$report" >/dev/null || fail 'resource command did not exit successfully'
	peak=$(sed -n 's/^peak_rss_kib=\([0-9][0-9]*\)$/\1/p' "$report")
	cpu=$(sed -n 's/^cpu_percent=\([0-9][0-9]*\)$/\1/p' "$report")
	startup=$(sed -n 's/^elapsed_ms=\([0-9][0-9]*\)$/\1/p' "$report")
	if [ -z "$peak" ] || [ -z "$cpu" ] || [ -z "$startup" ]; then
		fail 'resource report is incomplete'
	fi
	max_peak_kib=$(( $(value rss_peak_max_bytes) / 1024 ))
	[ "$peak" -le "$max_peak_kib" ] || fail "peak RSS exceeds budget: ${peak}KiB"
	[ "$cpu" -le "$(value cpu_probe_max_percent)" ] || fail "CPU exceeds budget: ${cpu}%"
	[ "$startup" -le $(( $(value startup_max_seconds) * 1000 )) ] || fail "startup exceeds budget: ${startup}ms"
fi

if [ -n "$package" ]; then package_checked=yes; else package_checked=no; fi
printf 'resource budgets passed: runtime_bytes=%s package_checked=%s\n' "$total" "$package_checked"
