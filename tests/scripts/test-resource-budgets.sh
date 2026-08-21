#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
budget="$repo_root/openwrt/files/usr/share/wificalling-location-gateway/resource-budget.conf"
profile="$repo_root/scripts/ci/profile-resource.sh"
gate="$repo_root/scripts/ci/verify-resource-budgets.sh"

[ -s "$budget" ]
[ -x "$profile" ]
[ -x "$gate" ]

for key in \
	format \
	runtime_binary_total_max_bytes \
	integrated_package_max_bytes \
	persistent_state_max_bytes \
	log_total_max_bytes \
	cache_total_max_bytes \
	max_profiles \
	startup_max_seconds \
	rss_idle_max_bytes \
	rss_peak_max_bytes \
	cpu_probe_max_percent; do
	grep -E "^${key}=" "$budget" >/dev/null || {
		echo "resource budget is missing $key" >&2
		exit 1
	}
done

tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-resource-budget.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

WLOC_RESOURCE_REPORT="$tmp/report.env" \
	"$profile" -- "$repo_root/tests/scripts/resource-fixture.sh"

grep -Fx 'status=pass' "$tmp/report.env" >/dev/null
grep -E '^elapsed_ms=[0-9]+$' "$tmp/report.env" >/dev/null
grep -E '^peak_rss_kib=[0-9]+$' "$tmp/report.env" >/dev/null
grep -E '^cpu_percent=[0-9]+$' "$tmp/report.env" >/dev/null

printf 'resource budget tests passed\n'
