#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
doc="$repo_root/docs/deployment/AX6S_DEPLOYMENT.md"
readme="$repo_root/README.md"
evidence="$repo_root/docs/testing/AX6S_RESOURCE_EVIDENCE.template.md"
fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}

[ -s "$doc" ] || fail 'AX6S deployment contract is missing'
[ -s "$readme" ] || fail 'README is missing'
[ -s "$evidence" ] || fail 'AX6S evidence template is missing'

grep -F 'cp -p /etc/config/wificalling-gateway /tmp/wificalling-gateway.backup' "$doc" >/dev/null ||
	fail 'migration must back up Gateway UCI before removal'
grep -F 'cp -p /etc/config/wloc-service /tmp/wloc-service.backup' "$doc" >/dev/null ||
	fail 'migration must back up WLOC UCI before removal'
grep -F 'Do not remove' "$doc" >/dev/null && grep -F '`sing-box` if it is the selected shared tiny/PassWall runtime' "$doc" >/dev/null ||
	fail 'migration must explicitly preserve the selected shared sing-box provider'
grep -F 'do not use' "$doc" >/dev/null && grep -F '`opkg remove --force-removal-of-dependent-packages`' "$doc" >/dev/null ||
	fail 'migration must prohibit forced dependency removal'

stop_line=$(grep -n -F '/etc/init.d/wloc-service stop' "$doc" | cut -d: -f1 | head -n1)
disable_line=$(grep -n -F '/etc/init.d/wificalling-gateway disable' "$doc" | cut -d: -f1 | head -n1)
remove_line=$(grep -n -F 'opkg remove luci-app-wificalling-location-gateway wloc-service wloc-ctl' "$doc" | cut -d: -f1 | head -n1)
space_after_remove=$(grep -n -F 'df -k /overlay /tmp' "$doc" | sed -n '2p' | cut -d: -f1)
install_line=$(grep -n -F 'opkg install /tmp/wificalling-location-gateway_1.2.0-1_aarch64_cortex-a53.ipk' "$doc" | cut -d: -f1 | head -n1)

[ -n "$stop_line" ] && [ -n "$disable_line" ] && [ -n "$remove_line" ] && [ -n "$space_after_remove" ] && [ -n "$install_line" ] ||
	fail 'migration steps are incomplete'
[ "$stop_line" -lt "$remove_line" ] || fail 'WLOC must stop before old packages are removed'
[ "$disable_line" -lt "$remove_line" ] || fail 'Gateway must be disabled before old packages are removed'
[ "$remove_line" -lt "$space_after_remove" ] || fail 'migration must recheck space after old packages are removed'
[ "$space_after_remove" -lt "$install_line" ] || fail 'new package must install after the post-removal space check'

if grep -E 'opkg remove[^\n]*(sing-box|passwall)' "$doc" >/dev/null; then
	fail 'migration removal command must not remove the shared provider'
fi
grep -F 'V2 reuses that executable' "$doc" >/dev/null ||
	fail 'deployment contract must document provider reuse'
grep -F '## 7. Rollback' "$doc" >/dev/null ||
	fail 'deployment contract must include rollback'

# The top-level installation guide is a user-facing copy of the release gate.
# Keep it from drifting back to the old, space-unaware direct-install advice.
if grep -F 'Do not run `opkg remove` first' "$readme" >/dev/null ||
	grep -F '不要先执行 `opkg remove`' "$readme" >/dev/null; then
	fail 'README must not instruct AX6S users to install over the old package'
fi
grep -F 'insufficient persistent storage' "$readme" >/dev/null ||
	fail 'README must document the AX6S storage constraint'
grep -F '空间不足以同时容纳旧应用包和集成包' "$readme" >/dev/null ||
	fail 'Chinese README must document the AX6S storage constraint'
grep -F 'Host/package gates passed; AX6S pending' "$readme" >/dev/null ||
	fail 'README must not claim unrecorded AX6S evidence'
grep -F '主机/构建门禁通过；AX6S 待测' "$readme" >/dev/null ||
	fail 'Chinese README must not claim unrecorded AX6S evidence'

grep -F '## Space-constrained migration evidence' "$evidence" >/dev/null ||
	fail 'AX6S evidence must include the space-constrained migration section'
grep -F 'Old application package names removed' "$evidence" >/dev/null ||
	fail 'AX6S evidence must record removed application packages'
grep -F 'Selected sing-box provider class retained' "$evidence" >/dev/null ||
	fail 'AX6S evidence must record retained provider class'
grep -F 'free-space bucket after old application removal' "$evidence" >/dev/null ||
	fail 'AX6S evidence must record post-removal free space'

printf '%s\n' 'AX6S migration contract tests passed'
