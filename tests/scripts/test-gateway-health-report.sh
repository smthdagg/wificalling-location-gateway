#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
health="$repo_root/openwrt/files/usr/sbin/wloc-health.sh"
node_health="$repo_root/openwrt/files/usr/libexec/wificalling-gateway/node-health.sh"

fail() {
	printf 'FAIL: %s\n' "$1" >&2
	exit 1
}

# Lite's /usr/bin/sing-box wrapper execs /tmp/sing-box-lite.  Health must
# recognize the final executable name instead of requiring the wrapper path.
grep -Eq "pgrep -f ['\"]sing-box\.\*.*run" "$health" ||
	fail 'Gateway health must detect the Lite sing-box wrapper target'
if grep -F "pgrep -f '/usr/bin/sing-box run'" "$health" >/dev/null; then
	fail 'Gateway health must not require the wrapper path in the process name'
fi

# monitor-loop passes a runtime output path; node-health must honor it so the
# Gateway status endpoint can read the same result that the monitor generated.
grep -F 'output=${2:-/www/wloc-node-status.json}' "$node_health" >/dev/null ||
	fail 'node-health must honor the output path supplied by monitor-loop'

printf 'gateway health report checks passed\n'
