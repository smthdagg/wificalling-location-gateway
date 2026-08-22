#!/bin/sh
set -eu
repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
doc="$repo_root/docs/deployment/AX6S_DEPLOYMENT.md"
readme="$repo_root/README.md"
[ -s "$doc" ] && [ -s "$readme" ]
grep -F '/tmp/wloc-service.backup' "$doc" >/dev/null
grep -F 'opkg remove luci-app-wificalling-location-gateway wificalling-location-gateway wloc-service wloc-ctl' "$doc" >/dev/null
grep -F 'sing-box tiny/lite' "$doc" >/dev/null
grep -F 'force-removal-of-dependent-packages' "$doc" >/dev/null
grep -F 'df -k /overlay /tmp' "$doc" >/dev/null
grep -F 'WiFi Calling + WLOC Gateway' "$doc" >/dev/null
grep -F 'Auto follow selected node' "$doc" >/dev/null
grep -F 'Component Update' "$doc" >/dev/null
grep -F 'wificalling-gateway' "$doc" >/dev/null
grep -F 'Gateway and WLOC' "$doc" >/dev/null
echo 'AX6S migration contract tests passed'
