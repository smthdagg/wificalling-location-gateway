#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
builder="$repo_root/scripts/build-luci-ipk.sh"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-standalone-package-test.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

printf '#!/bin/sh
exit 0
' > "$tmp/wloc-service"
printf '#!/bin/sh
exit 0
' > "$tmp/wloc-ctl"
chmod 0755 "$tmp/wloc-service" "$tmp/wloc-ctl"
version="2.0.0-standalone-test"
output=$(WLOC_SERVICE_BIN="$tmp/wloc-service" WLOC_CTL_BIN="$tmp/wloc-ctl" "$builder" "$version" ax6s-standalone)
[ -f "$output" ]
test -x "$repo_root/openwrt/files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh"
mkdir -p "$tmp/result"
tar -xf "$output" -C "$tmp/result"
control=$(tar -xOf "$tmp/result/control.tar.gz" ./control)
conffiles=$(tar -xOf "$tmp/result/control.tar.gz" ./conffiles)
postinst=$(tar -xOf "$tmp/result/control.tar.gz" ./postinst)
data_members=$(tar -tzf "$tmp/result/data.tar.gz")

printf '%s
' "$control" | grep -Fx 'Package: wificalling-location-gateway' >/dev/null
printf '%s
' "$control" | grep -Fx 'Architecture: aarch64_cortex-a53' >/dev/null
printf '%s
' "$control" | grep -Fx 'Description: Standalone WLOC location service with unified LuCI.' >/dev/null
printf '%s
' "$control" | grep -F 'X-WLOC-Product: wificalling-location-gateway/v2' >/dev/null
printf '%s
' "$control" | grep -F 'X-WLOC-Api: wloc.service/v2' >/dev/null
printf '%s
' "$control" | grep -F 'Provides: wloc-service' >/dev/null
if printf '%s
' "$control" | grep -E 'Wi-Fi Calling Gateway|luci-app-wificalling-gateway|X-WFC'; then
  echo 'standalone metadata contains Gateway coupling' >&2
  exit 1
fi
printf '%s
' "$conffiles" | grep -Fx '/etc/config/wloc-service' >/dev/null
if printf '%s
' "$conffiles" | grep -F 'wificalling-gateway'; then exit 1; fi
printf '%s
' "$postinst" | grep -F 'install sing-box tiny/lite or a PassWall sing-box provider' >/dev/null
printf '%s
' "$data_members" | grep -Fx './etc/config/wloc-service' >/dev/null
printf '%s
' "$data_members" | grep -Fx './usr/sbin/wloc-service' >/dev/null
printf '%s
' "$data_members" | grep -Fx './usr/sbin/wloc-ctl' >/dev/null
printf '%s
' "$data_members" | grep -Fx './usr/libexec/wificalling-location-gateway/unified-supervisor.sh' >/dev/null
if printf '%s
' "$data_members" | grep -E 'wificalling-gateway|view/wificalling-gateway'; then
  echo 'standalone package contains Gateway payload' >&2
  exit 1
fi
echo 'standalone AX6S package tests passed'
