#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
health="$repo_root/openwrt/files/usr/sbin/wloc-health.sh"
node_health="$repo_root/openwrt/files/usr/libexec/wificalling-gateway/node-health.sh"
init="$repo_root/openwrt/files/etc/init.d/wificalling-gateway"

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
grep -F '"$RUNDIR/nodes" "/www/wloc-node-status.json"' "$init" >/dev/null ||
	fail 'monitor-loop must publish node health to the LuCI-readable status file'

# r7/r8 use the single-process proxy-path checker instead of the retired
# standalone WireGuard handshake helper.  The health badge must recognize the
# maintained implementation rather than report a false red after every boot.
grep -F "grep -q 'node_icmp_test' /usr/libexec/wificalling-gateway/node-health.sh && patch_health=1" "$health" >/dev/null ||
	fail 'Gateway health must recognize the maintained ICMP node health check'

grep -F '"state":"\(reachable\|tcp_reachable' "$health" >/dev/null ||
	fail 'Gateway health must count reachable node results as healthy'
grep -F '"state":"\(unreachable\|handshake_failed' "$health" >/dev/null ||
	fail 'Gateway health must count unreachable node results as down'

printf 'gateway health report checks passed\n'
