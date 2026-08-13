#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
builder="$repo_root/scripts/build-luci-ipk.sh"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-standalone-package-test.XXXXXX")
built_output=
cleanup() {
	rm -rf "$tmp"
	[ -z "$built_output" ] || rm -f "$built_output"
}
trap cleanup EXIT HUP INT TERM

fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}

mkdir -p "$tmp/gateway/control" "$tmp/gateway/data/etc/config" \
	"$tmp/gateway/data/etc/init.d" "$tmp/gateway/data/www/luci-static/resources/view/wificalling-gateway"
cat > "$tmp/gateway/control/control" <<'EOF'
Package: luci-app-wificalling-gateway
Version: 1.7.3-1
Architecture: all
License: MIT
EOF
printf '%s\n' '/etc/config/wificalling-gateway' > "$tmp/gateway/control/conffiles"
printf '%s\n' 'config main main' > "$tmp/gateway/data/etc/config/wificalling-gateway"
printf '%s\n' '#!/bin/sh' > "$tmp/gateway/data/etc/init.d/wificalling-gateway"
printf '%s\n' "'use strict';" > "$tmp/gateway/data/www/luci-static/resources/view/wificalling-gateway/overview.js"
chmod 0755 "$tmp/gateway/data/etc/init.d/wificalling-gateway"
printf '2.0\n' > "$tmp/gateway/debian-binary"
(cd "$tmp/gateway/control" && tar -czf "$tmp/gateway/control.tar.gz" .)
(cd "$tmp/gateway/data" && tar -czf "$tmp/gateway/data.tar.gz" .)
(cd "$tmp/gateway" && tar -czf "$tmp/gateway.ipk" debian-binary control.tar.gz data.tar.gz)

printf '#!/bin/sh\nexit 0\n' > "$tmp/wloc-service"
printf '#!/bin/sh\nexit 0\n' > "$tmp/wloc-ctl"
chmod 0755 "$tmp/wloc-service" "$tmp/wloc-ctl"
gateway_sha=$(shasum -a 256 "$tmp/gateway.ipk" | awk '{print $1}')
version="0.1.0-4-standalone-test"

output=$(
	WLOC_SERVICE_BIN="$tmp/wloc-service" \
	WLOC_CTL_BIN="$tmp/wloc-ctl" \
	GATEWAY_IPK="$tmp/gateway.ipk" \
	GATEWAY_IPK_SHA256="$gateway_sha" \
	"$builder" "$version" ax6s-standalone
)
built_output=$output
[ -f "$output" ] || fail 'standalone builder did not create an IPK'

mkdir -p "$tmp/result"
tar -xf "$output" -C "$tmp/result"
control=$(tar -xOf "$tmp/result/control.tar.gz" ./control)
conffiles=$(tar -xOf "$tmp/result/control.tar.gz" ./conffiles)
postinst=$(tar -xOf "$tmp/result/control.tar.gz" ./postinst)
data_members=$(tar -tzf "$tmp/result/data.tar.gz")

printf '%s\n' "$output" | grep -F "/wificalling-location-gateway_${version}_aarch64_cortex-a53.ipk" >/dev/null ||
	fail 'standalone package filename must use the project name and identify the AX6S architecture'
printf '%s\n' "$control" | grep -Fx 'Package: wificalling-location-gateway' >/dev/null ||
	fail 'standalone package metadata must use the project name'
printf '%s\n' "$control" | grep -Fx 'Architecture: aarch64_cortex-a53' >/dev/null ||
	fail 'standalone package metadata must identify the AX6S runtime architecture'
printf '%s\n' "$control" | grep -Fx 'Description: Complete Wi-Fi Calling Gateway 1.7 and WLOC service with unified LuCI.' >/dev/null ||
	fail 'standalone package description must identify the complete integrated product'
printf '%s\n' "$control" | grep -F 'Provides: luci-app-wificalling-location-gateway, luci-app-wificalling-gateway, wloc-service' >/dev/null ||
	fail 'standalone package must provide both bundled components'
printf '%s\n' "$control" | grep -F 'Replaces: luci-app-wificalling-location-gateway, luci-app-wificalling-gateway, wloc-service' >/dev/null ||
	fail 'standalone package must support upgrades from the former component package names'
printf '%s\n' "$control" | grep -F 'Depends: luci-base, rpcd-mod-rpcsys, sing-box, nftables, firewall4, kmod-nft-tproxy, kmod-nft-socket, ip-full' >/dev/null ||
	fail 'standalone package must depend only on system packages'
if printf '%s\n' "$control" | grep '^Depends:.*luci-app-wificalling-gateway' >/dev/null; then
	fail 'standalone package must not depend on the separate Gateway package'
fi
printf '%s\n' "$conffiles" | grep -Fx '/etc/config/wificalling-gateway' >/dev/null ||
	fail 'Gateway configuration must be preserved across reinstalls'
printf '%s\n' "$conffiles" | grep -Fx '/etc/config/wloc-service' >/dev/null ||
	fail 'WLOC configuration must be preserved across reinstalls'
printf '%s\n' "$postinst" | grep -F 'mkdir -p /var/run/wificalling-gateway' >/dev/null ||
	fail 'standalone post-install must create the volatile Gateway runtime directory before restart'
printf '%s\n' "$postinst" | grep -F 'chmod 0700 /var/run/wificalling-gateway' >/dev/null ||
	fail 'standalone post-install must restrict the Gateway runtime directory'
runtime_line=$(printf '%s\n' "$postinst" | grep -n -F 'mkdir -p /var/run/wificalling-gateway' | cut -d: -f1)
restart_line=$(printf '%s\n' "$postinst" | grep -n -F '/etc/init.d/wificalling-gateway restart' | cut -d: -f1)
[ "$runtime_line" -lt "$restart_line" ] ||
	fail 'standalone post-install must create the Gateway runtime directory before restart'
for member in \
	'./etc/config/wificalling-gateway' \
	'./etc/init.d/wificalling-gateway' \
	'./etc/config/wloc-service' \
	'./etc/init.d/wloc-service' \
	'./usr/sbin/wloc-service' \
	'./usr/sbin/wloc-ctl'; do
	printf '%s\n' "$data_members" | grep -Fx "$member" >/dev/null ||
		fail "standalone package is missing $member"
done

if GATEWAY_IPK="$tmp/gateway.ipk" GATEWAY_IPK_SHA256=deadbeef \
	WLOC_SERVICE_BIN="$tmp/wloc-service" WLOC_CTL_BIN="$tmp/wloc-ctl" \
	"$builder" "$version-bad-sha" ax6s-standalone >"$tmp/out" 2>"$tmp/err"; then
	fail 'standalone builder must reject an unpinned Gateway package'
fi
grep -F 'Gateway IPK SHA-256 mismatch' "$tmp/err" >/dev/null ||
	fail 'Gateway digest rejection must be explicit'

printf '%s\n' 'standalone AX6S package tests passed'
