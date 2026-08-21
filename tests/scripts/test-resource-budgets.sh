#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
budget="$repo_root/openwrt/files/usr/share/wificalling-location-gateway/resource-budget.conf"
profile="$repo_root/scripts/ci/profile-resource.sh"
gate="$repo_root/scripts/ci/verify-resource-budgets.sh"
package_gate="$repo_root/scripts/ci/verify-package-budget.sh"

[ -s "$budget" ]
[ -x "$profile" ]
[ -x "$gate" ]
[ -x "$package_gate" ]

for key in \
	format \
	runtime_binary_total_max_bytes \
	integrated_package_max_bytes \
	persistent_state_max_bytes \
	log_total_max_bytes \
	cache_total_max_bytes \
	max_profiles \
	startup_max_seconds \
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

timeout_report="$tmp/timeout-report.env"
if WLOC_RESOURCE_TIMEOUT_SECONDS=1 WLOC_RESOURCE_REPORT="$timeout_report" \
	"$profile" -- sleep 2 >/dev/null 2>&1; then
	echo 'resource profiler accepted a timed-out command' >&2
	exit 1
fi
grep -E '^command_status=[1-9][0-9]*$' "$timeout_report" >/dev/null

if [ -r /proc/self/status ] && [ -r /proc/self/stat ]; then
	WLOC_RESOURCE_FORCE_PROCFS=1 \
		WLOC_RESOURCE_REPORT="$tmp/procfs-report.env" \
		"$profile" -- "$repo_root/tests/scripts/resource-fixture.sh"
	grep -Fx 'status=pass' "$tmp/procfs-report.env" >/dev/null
	grep -E '^peak_rss_kib=[0-9]+$' "$tmp/procfs-report.env" >/dev/null
fi

mkdir -p "$tmp/bins"
for binary in wloc-gateway-spike wloc-service wloc-ctl; do
	cp "$repo_root/tests/scripts/resource-fixture.sh" "$tmp/bins/$binary"
done
package="$tmp/package.ipk"
cp "$repo_root/tests/scripts/resource-fixture.sh" "$package"
"$package_gate" "$package" >/dev/null
gate_report="$tmp/gate-report.env"
sed 's/^cpu_percent=.*/cpu_percent=1/' "$tmp/report.env" > "$gate_report"
WLOC_RESOURCE_ARTIFACT_DIR="$tmp/bins" \
	WLOC_PACKAGE_ARTIFACT="$package" \
	WLOC_RESOURCE_REPORT="$gate_report" \
	"$gate"

oversized="$tmp/bins/wloc-service"
limit=$(sed -n 's/^runtime_binary_total_max_bytes=//p' "$budget")
dd if=/dev/zero of="$oversized" bs=1 count=$((limit + 1)) >/dev/null 2>&1
if WLOC_RESOURCE_ARTIFACT_DIR="$tmp/bins" "$gate" >/dev/null 2>&1; then
	echo 'resource gate accepted oversized runtime binaries' >&2
	exit 1
fi

package_limit=$(sed -n 's/^integrated_package_max_bytes=//p' "$budget")
oversized_package="$tmp/oversized.ipk"
dd if=/dev/zero of="$oversized_package" bs=1m \
	count=$((package_limit / 1048576 + 1)) >/dev/null 2>&1
if WLOC_RESOURCE_ARTIFACT_DIR="$tmp/bins" WLOC_PACKAGE_ARTIFACT="$oversized_package" "$gate" >/dev/null 2>&1; then
	echo 'resource gate accepted oversized package' >&2
	exit 1
fi
if "$package_gate" "$oversized_package" >/dev/null 2>&1; then
	echo 'package gate accepted oversized package' >&2
	exit 1
fi

peak_limit=$(( $(sed -n 's/^rss_peak_max_bytes=//p' "$budget") / 1024 + 1 ))
cpu_limit=$(( $(sed -n 's/^cpu_probe_max_percent=//p' "$budget") + 1 ))
startup_limit=$(( $(sed -n 's/^startup_max_seconds=//p' "$budget") * 1000 + 1 ))
for field in peak_rss_kib cpu_percent elapsed_ms; do
	case "$field" in
		peak_rss_kib) value=$peak_limit ;;
		cpu_percent) value=$cpu_limit ;;
		elapsed_ms) value=$startup_limit ;;
	esac
	bad_report="$tmp/bad-$field.env"
	sed "s/^$field=.*/$field=$value/" "$gate_report" > "$bad_report"
	if WLOC_RESOURCE_ARTIFACT_DIR="$tmp/bins" WLOC_RESOURCE_REPORT="$bad_report" "$gate" >/dev/null 2>&1; then
		echo "resource gate accepted over-budget $field" >&2
		exit 1
	fi
done

failed_report="$tmp/failed-report.env"
sed -e 's/^status=.*/status=fail/' -e 's/^command_status=.*/command_status=1/' "$gate_report" > "$failed_report"
if WLOC_RESOURCE_ARTIFACT_DIR="$tmp/bins" WLOC_RESOURCE_REPORT="$failed_report" "$gate" >/dev/null 2>&1; then
	echo 'resource gate accepted a failed resource command' >&2
	exit 1
fi

printf 'resource budget tests passed\n'
