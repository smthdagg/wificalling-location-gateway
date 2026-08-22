#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
builder="$repo_root/scripts/openwrt/build-release-packages.sh"
matrix="$repo_root/scripts/openwrt/verify-docker-matrix.sh"
runtime_builder="$repo_root/scripts/openwrt/build-x86_64-runtime.sh"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-package-test.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

[ -x "$builder" ] && [ -x "$matrix" ] && [ -x "$runtime_builder" ]
grep -F 'external application UCI or package' "$builder" >/dev/null
grep -F 'wloc-service' "$builder" >/dev/null
grep -F 'wificalling-gateway' "$builder" >/dev/null
grep -F 'wificalling-gateway/overview' "$repo_root/openwrt/files/usr/share/luci/menu.d/luci-app-wificalling-location-gateway.json" >/dev/null
grep -F -- '--ax6s-package' "$builder" >/dev/null
grep -F 'expected three integrated packages' "$builder" >/dev/null
grep -F 'X-WLOC-Product' "$repo_root/scripts/build-luci-ipk.sh" >/dev/null
grep -F 'X-WLOC-Target: x86/64' "$builder" >/dev/null
grep -F 'X-WLOC-OpenWrt: 24.10+' "$builder" >/dev/null
grep -F 'X-WLOC-Package-Format: ipk' "$builder" >/dev/null
grep -F 'DEPENDS:=' "$builder" >/dev/null
for dependency in '+luci-base' '+rpcd-mod-rpcsys' '+nftables' '+firewall4' '+kmod-nft-tproxy' '+kmod-nft-socket' '+ip-full'; do
  grep -F "$dependency" "$builder" >/dev/null
done
grep -F "wificalling-location-gateway*.manifest" "$builder" >/dev/null
grep -F "wificalling-location-gateway*.sig" "$builder" >/dev/null
grep -F 'WLOC_UPDATE_SIGNING_KEY is required' "$builder" >/dev/null
grep -F '/etc/init.d/wificalling-location-gateway restart' "$matrix" >/dev/null
if grep -E -- '--gateway-ipk|GATEWAY_IPK' "$builder" >/dev/null; then
  echo 'release builder still exposes a Gateway package input' >&2
  exit 1
fi

printf '#!/bin/sh
exit 0
' > "$tmp/wloc-service"
printf '#!/bin/sh
exit 0
' > "$tmp/wloc-ctl"
chmod 0755 "$tmp/wloc-service" "$tmp/wloc-ctl"
plan=$("$builder" --plan --arch x86_64 --service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl")
printf '%s
' "$plan" | grep -F 'wificalling-location-gateway_2.0.0-r1_x86_64.ipk' >/dev/null
printf '%s
' "$plan" | grep -F 'wificalling-location-gateway-2.0.0-r1.apk (arch: x86_64)' >/dev/null
printf '%s
' "$plan" | grep -F 'ghcr.io/openwrt/sdk:x86_64-24.10.8@sha256:b28d5e4087dbd3f815a8bf5440a11e54e6bbd3d7400c3729d872e7940a4a77c1' >/dev/null
printf '%s
' "$plan" | grep -F 'ghcr.io/openwrt/sdk:x86_64-25.12.3@sha256:a0ab488698b70d6585dc35bebb77b3f6d9523fd68873fab78a1bd19cc123cd0f' >/dev/null

if "$builder" --plan --arch all --service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl" >"$tmp/out" 2>"$tmp/err"; then
  exit 1
fi
grep -F 'runtime architecture must not be all or noarch' "$tmp/err" >/dev/null
if "$builder" --out-dir /tmp --arch x86_64 --service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl" >"$tmp/out" 2>"$tmp/err"; then
  exit 1
fi
grep -F 'dedicated openwrt-release directory' "$tmp/err" >/dev/null

matrix_plan=$("$matrix" --plan --dist-dir "$tmp")
printf '%s
' "$matrix_plan" | grep -F 'Redmi AX6S / OpenWrt 24.10.5|opkg' >/dev/null
printf '%s
' "$matrix_plan" | grep -F 'OpenWrt 25.12.3|apk' >/dev/null
runtime_plan=$("$runtime_builder" --plan --out-dir "$tmp")
printf '%s
' "$runtime_plan" | grep -F 'x86_64-unknown-linux-musl' >/dev/null
echo 'integrated Gateway/WLOC release packaging tests passed'
