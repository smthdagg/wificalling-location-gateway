#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
builder="$repo_root/scripts/build-luci-ipk.sh"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-standalone-package-test.XXXXXX")
built_outputs=
cleanup() {
	rm -rf "$tmp"
	for built_output in $built_outputs; do
		rm -f "$built_output"
	done
}
trap cleanup EXIT HUP INT TERM

fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}

mkdir -p "$tmp/gateway/control" "$tmp/gateway/data/etc/config" \
	"$tmp/gateway/data/etc/init.d" "$tmp/gateway/data/www/luci-static/resources/view/wificalling-gateway" \
	"$tmp/gateway/data/usr/share/luci/menu.d"
cat > "$tmp/gateway/control/control" <<'EOF'
Package: wificalling-location-gateway
Version: 1.3.0-r1
Architecture: all
License: MIT
EOF
printf '%s\n' '/etc/config/wificalling-gateway' > "$tmp/gateway/control/conffiles"
printf '%s\n' 'config main main' > "$tmp/gateway/data/etc/config/wificalling-gateway"
printf '%s\n' '#!/bin/sh' > "$tmp/gateway/data/etc/init.d/wificalling-gateway"
printf '%s\n' "'use strict';" > "$tmp/gateway/data/www/luci-static/resources/view/wificalling-gateway/overview.js"
printf '%s\n' '{"admin/services/wificalling-gateway":{"title":"Wi-Fi Calling Gateway"}}' > \
	"$tmp/gateway/data/usr/share/luci/menu.d/luci-app-wificalling-gateway.json"
# The old payload is intentional: the stable 1.3.0-r1 upgrade base must be
# overlaid with the current maintained files rather than trusted blindly.
mkdir -p "$tmp/gateway/data/usr/libexec/wificalling-gateway"
cat > "$tmp/gateway/data/usr/libexec/wificalling-gateway/compiler.sh" <<'COMPILER'
#!/bin/sh
      s=s ",\"peers\":[{\"address\":" q(f[4]) ",\"port\":" f[5] ",\"public_key\":" q(f[13]) ",\"allowed_ips\":[\"0.0.0.0/0\"]"
      s=s ",\"private_key\":" q(f[21]) ",\"peer_public_key\":" q(f[13]) ",\"local_address\":[" q(f[22]) "]"
      if (!node_proto[$3]) fail("device references unknown node: " $3)
COMPILER
cat > "$tmp/gateway/data/usr/libexec/wificalling-gateway/node-health.sh" <<'HEALTH'
#!/bin/sh
output=${2:-/var/run/wificalling-gateway/node-status.json}
json_escape() {
	printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}
	printf '{"generated_at":%s,"note":"ICMP ping only; this is not a proxy protocol handshake.","nodes":[' "$(date +%s)"
		[ "$first" -eq 1 ] || printf ','
		printf '{"id":"%s","label":"%s","protocol":"%s","server":"%s","port":%s,"state":"%s","measurement":"%s","ping_ms":%s}' \
			"$(json_escape "$id")" "$(json_escape "$label")" "$(json_escape "$protocol")" \
			"$(json_escape "$server")" "$port" "$state" "$measurement" "$ping_json"
HEALTH
chmod 0755 "$tmp/gateway/data/usr/libexec/wificalling-gateway/node-health.sh"
cat >> "$tmp/gateway/data/etc/init.d/wificalling-gateway" <<'INITD'
	config_get private_key "$s" private_key; config_get local_address "$s" local_address; config_get reserved "$s" reserved; config_get mtu "$s" mtu
	printf 'node|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s\n' "$s" "$protocol" "$server" "$port" "$credential" "$sni" "$insecure" "$alpn" "$auxiliary" "$congestion" "$udp_mode" "$public_key" "$short_id" "$fingerprint" "$security" "$transport" "$path" "$host" "$pin_sha256" "$private_key" "$local_address" "$reserved" "$mtu" >> "$RUNDIR/normalized.conf"
INITD
chmod 0755 "$tmp/gateway/data/etc/init.d/wificalling-gateway" "$tmp/gateway/data/usr/libexec/wificalling-gateway/compiler.sh"
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
built_outputs="$built_outputs $output"
[ -f "$output" ] || fail 'standalone builder did not create an IPK'

mkdir -p "$tmp/result"
tar -xf "$output" -C "$tmp/result"
control=$(tar -xOf "$tmp/result/control.tar.gz" ./control)
conffiles=$(tar -xOf "$tmp/result/control.tar.gz" ./conffiles)
postinst=$(tar -xOf "$tmp/result/control.tar.gz" ./postinst)
preinst=$(tar -xOf "$tmp/result/control.tar.gz" ./preinst)
prerm=$(tar -xOf "$tmp/result/control.tar.gz" ./prerm)
data_members=$(tar -tzf "$tmp/result/data.tar.gz")

printf '%s\n' "$output" | grep -F "/wificalling-location-gateway_${version}_aarch64_cortex-a53.ipk" >/dev/null ||
	fail 'standalone package filename must use the project name and identify the AX6S architecture'
printf '%s\n' "$control" | grep -Fx 'Package: wificalling-location-gateway' >/dev/null ||
	fail 'standalone package metadata must use the project name'
printf '%s\n' "$control" | grep -Fx 'Architecture: aarch64_cortex-a53' >/dev/null ||
	fail 'standalone package metadata must identify the AX6S runtime architecture'
printf '%s\n' "$control" | grep -Fx 'Description: Complete Wi-Fi Calling Gateway and WLOC service with unified LuCI.' >/dev/null ||
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
printf '%s\n' "$postinst" | grep -F 'rm -f /tmp/luci-indexcache.*' >/dev/null ||
	fail 'standalone post-install must invalidate every LuCI menu cache variant'
check_lifecycle() {
	content=$1
	phase=$2
	printf '%s\n' "$content" | grep -F '/etc/init.d/wloc-service stop' >/dev/null ||
		fail "standalone $phase must stop WLOC before package files change"
	printf '%s\n' "$content" | grep -F '/etc/init.d/wificalling-gateway stop' >/dev/null ||
		fail "standalone $phase must stop Gateway before package files change"
	if printf '%s\n' "$content" | grep -F 'killall -q sing-box' >/dev/null; then
		fail "standalone $phase must not kill unrelated sing-box services"
	fi
}
check_lifecycle "$preinst" preinst
check_lifecycle "$prerm" prerm
for lifecycle in preinst prerm; do
	tar -tvzf "$tmp/result/control.tar.gz" | grep -E "^-rwxr-xr-x .* \\./$lifecycle$" >/dev/null ||
		fail "standalone $lifecycle must be executable in the control archive"
done
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
if printf '%s\n' "$data_members" | grep -Fx './usr/share/luci/menu.d/luci-app-wificalling-gateway.json' >/dev/null; then
	fail 'integrated package must not expose the standalone Gateway LuCI menu'
fi
# The wireguard pre_shared_key patch must have been applied to the merged
# Gateway payload (compiler.sh endpoint + legacy branches, init.d field).
mkdir -p "$tmp/result/data"
tar -xzf "$tmp/result/data.tar.gz" -C "$tmp/result/data"
view_suffix=$(printf '%s' "$version" | tr '.-' '__')
versioned_view="$tmp/result/data/www/luci-static/resources/view/wificalling-gateway/wfc_overview_fix_$view_suffix.js"
versioned_import="$tmp/result/data/www/luci-static/resources/wificalling-gateway/node-import_fix_$view_suffix.js"
[ -f "$versioned_view" ] || fail 'standalone package must install a versioned WFC view'
[ -f "$versioned_import" ] || fail 'standalone package must install a versioned node importer'
grep -F "require wificalling-gateway.node-import_fix_$view_suffix as nodeImport" \
	"$versioned_view" >/dev/null ||
	fail 'versioned WFC view must load the versioned node importer'
grep -F 'if (f[25]!="") s=s ",\"pre_shared_key\":" q(f[25])' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/compiler.sh" >/dev/null 2>&1 ||
	fail 'standalone package must patch compiler.sh with pre_shared_key support'
[ "$(grep -c 'pre_shared_key' "$tmp/result/data/usr/libexec/wificalling-gateway/compiler.sh")" -eq 2 ] ||
	fail 'standalone package must patch both wireguard compiler styles'
grep -F 'config_get pre_shared_key "$s" pre_shared_key' \
	"$tmp/result/data/etc/init.d/wificalling-gateway" >/dev/null ||
	fail 'standalone package must patch init.d with the pre_shared_key field'
grep -F 'device_guard_marker' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/compiler.sh" >/dev/null ||
	fail 'standalone package must skip devices with stale node references'
grep -F 'fail("device references unknown node: " $3)' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/compiler.sh" >/dev/null &&
	fail 'standalone package must not keep the fail-hard unknown-node device path'
grep -F 'wg_handshake_test' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh" >/dev/null ||
	fail 'standalone package must patch node-health.sh with the wireguard handshake test'
[ "$(grep -c 'wg_handshake_test' "$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh")" -ge 2 ] ||
	fail 'standalone package handshake patch must define and call wg_handshake_test'
grep -F '[ -n "$reserved" ]' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh" >/dev/null ||
	fail 'standalone package handshake patch must forward the reserved field'
grep -F 'reason_json=' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh" >/dev/null ||
	fail 'standalone package handshake patch must report a failure reason'
grep -F 'config_missing' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh" >/dev/null ||
	fail 'standalone package handshake patch must distinguish missing key material'
grep -F 'md5sum | cut -c1-4' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh" >/dev/null ||
	fail 'standalone package handshake patch must derive the probe port from a busybox-safe hash'
grep -F 'wg-health.lock' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh" >/dev/null ||
	fail 'standalone package handshake patch must serialize concurrent handshake tests'
grep -F 'kill -0' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh" >/dev/null ||
	fail 'standalone package handshake patch must reclaim stale test locks'
grep -F 'compact_status_marker' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh" >/dev/null ||
	fail 'standalone package must compact the node-status.json output'
[ "$(grep -c '"id":"%s","state":"%s","measurement":"%s","ping_ms":%s' "$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh")" -eq 1 ] ||
	fail 'standalone package compact output must drop the unused fields'
grep -F '"reason":%s' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh" >/dev/null ||
	fail 'standalone package compact output must include the handshake failure reason'
# The manual per-node connection test helper must ship and be wired into
# rpcd so the LuCI "Test connection" button can ask for a fresh check.
grep -F 'wg_handshake_test' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-test.sh" >/dev/null ||
	fail 'standalone package must ship the manual node test helper'
grep -F 'node_test' \
	"$tmp/result/data/usr/libexec/rpcd/luci.wloc" >/dev/null ||
	fail 'standalone package rpcd plugin must expose the node_test method'
grep -F 'node_test' \
	"$tmp/result/data/usr/share/rpcd/acl.d/luci-app-wificalling-location-gateway.json" >/dev/null ||
	fail 'standalone package ACL must whitelist the node_test method'
[ -x "$tmp/result/data/usr/libexec/wificalling-gateway/node-test.sh" ] ||
	fail 'standalone package node-test.sh must be executable'
grep -F '"note":"ICMP ping only' \
	"$tmp/result/data/usr/libexec/wificalling-gateway/node-health.sh" >/dev/null &&
	fail 'standalone package compact output must drop the note field'

# The Lite variant owns an architecture-matched sing-box binary, replaces the
# standard runtime through package metadata, and preserves the same UCI files.
mkdir -p "$tmp/mock-bin"
cat > "$tmp/mock-bin/file" <<'FILE'
#!/bin/sh
printf '%s: ELF 64-bit LSB executable, ARM aarch64, version 1 (SYSV)\n' "$1"
FILE
chmod 0755 "$tmp/mock-bin/file"
printf 'mock aarch64 sing-box Lite\n' > "$tmp/sing-box-lite"
chmod 0755 "$tmp/sing-box-lite"
tiny_sha=$(shasum -a 256 "$tmp/sing-box-lite" | awk '{print $1}')
lite_version="0.1.0-4-lite-test"
lite_output=$(
	PATH="$tmp/mock-bin:$PATH" \
	WLOC_SERVICE_BIN="$tmp/wloc-service" \
	WLOC_CTL_BIN="$tmp/wloc-ctl" \
	GATEWAY_IPK="$tmp/gateway.ipk" \
	GATEWAY_IPK_SHA256="$gateway_sha" \
	SINGBOX_LITE_BIN="$tmp/sing-box-lite" \
	SINGBOX_LITE_SHA256="$tiny_sha" \
	"$builder" "$lite_version" ax6s-lite
)
built_outputs="$built_outputs $lite_output"
mkdir -p "$tmp/lite-result/data"
tar -xf "$lite_output" -C "$tmp/lite-result"
lite_control=$(tar -xOf "$tmp/lite-result/control.tar.gz" ./control)
lite_conffiles=$(tar -xOf "$tmp/lite-result/control.tar.gz" ./conffiles)
tar -xzf "$tmp/lite-result/data.tar.gz" -C "$tmp/lite-result/data"

printf '%s\n' "$lite_control" | grep -Fx 'Package: wificalling-location-gateway-lite' >/dev/null ||
	fail 'Lite package must have an explicit package identity'
printf '%s\n' "$lite_control" | grep -F 'Provides: wificalling-location-gateway, sing-box' >/dev/null ||
	fail 'Lite package must provide the integrated product and sing-box runtime'
printf '%s\n' "$lite_control" | grep -F 'Conflicts: wificalling-location-gateway, sing-box' >/dev/null ||
	fail 'Lite package must conflict with the standard product and sing-box package'
if printf '%s\n' "$lite_control" | grep '^Depends:.*sing-box' >/dev/null; then
	fail 'Lite package must not depend on the standard sing-box package'
fi
printf '%s\n' "$lite_control" | grep -F 'kmod-inet-diag, kmod-netlink-diag, kmod-tun' >/dev/null ||
	fail 'Lite package must carry the runtime kernel dependencies'
for conffile in /etc/config/wificalling-gateway /etc/config/wloc-service; do
	printf '%s\n' "$lite_conffiles" | grep -Fx "$conffile" >/dev/null ||
		fail "Lite package must preserve $conffile"
done
grep -F '/tmp/sing-box-lite' "$tmp/lite-result/data/usr/bin/sing-box" >/dev/null ||
	fail 'Lite package must install the transparent tmpfs runtime wrapper'
gzip -dc "$tmp/lite-result/data/usr/share/wificalling-location-gateway/sing-box-lite.gz" \
	> "$tmp/lite-result/sing-box-lite.unpacked"
cmp "$tmp/sing-box-lite" "$tmp/lite-result/sing-box-lite.unpacked" >/dev/null ||
	fail 'Lite package must persist the exact hash-pinned sing-box runtime in compressed form'
[ "$(cat "$tmp/lite-result/data/usr/share/wificalling-location-gateway/runtime-variant")" = lite ] ||
	fail 'Lite package must install its runtime marker'
if grep -F 'GOMEMLIMIT=' "$tmp/lite-result/data/etc/init.d/wificalling-gateway" >/dev/null; then
	fail 'Lite package must not apply an artificial sing-box memory ceiling'
fi
grep -F 'GOMAXPROCS=1' "$tmp/lite-result/data/etc/init.d/wificalling-gateway" >/dev/null ||
	fail 'Lite package must retain single-worker scheduling'
grep -F 'GOGC=75' "$tmp/lite-result/data/etc/init.d/wificalling-gateway" >/dev/null ||
	fail 'Lite package must retain the moderate GC target'

# The retired standalone 1.7 package must never be accepted as a baseline.
mkdir -p "$tmp/legacy"
cp -R "$tmp/gateway/." "$tmp/legacy/"
sed 's/^Package: .*/Package: luci-app-wificalling-gateway/; s/^Version: .*/Version: 1.7.3-1/' \
	"$tmp/legacy/control/control" > "$tmp/legacy/control/control.next"
mv "$tmp/legacy/control/control.next" "$tmp/legacy/control/control"
(cd "$tmp/legacy/control" && tar -czf "$tmp/legacy/control.tar.gz" .)
(cd "$tmp/legacy" && tar -czf "$tmp/legacy.ipk" debian-binary control.tar.gz data.tar.gz)
legacy_sha=$(shasum -a 256 "$tmp/legacy.ipk" | awk '{print $1}')
if GATEWAY_IPK="$tmp/legacy.ipk" GATEWAY_IPK_SHA256="$legacy_sha" \
	WLOC_SERVICE_BIN="$tmp/wloc-service" WLOC_CTL_BIN="$tmp/wloc-ctl" \
	"$builder" "$version-legacy" ax6s-standalone >"$tmp/out" 2>"$tmp/err"; then
	fail 'standalone builder must reject the retired 1.7 package baseline'
fi
grep -F 'stable integrated 1.3.0-r1 release' "$tmp/err" >/dev/null ||
	fail 'legacy baseline rejection must explain the stable integrated requirement'

if GATEWAY_IPK="$tmp/gateway.ipk" GATEWAY_IPK_SHA256=deadbeef \
	WLOC_SERVICE_BIN="$tmp/wloc-service" WLOC_CTL_BIN="$tmp/wloc-ctl" \
	"$builder" "$version-bad-sha" ax6s-standalone >"$tmp/out" 2>"$tmp/err"; then
	fail 'standalone builder must reject an unpinned Gateway package'
fi
grep -F 'Gateway IPK SHA-256 mismatch' "$tmp/err" >/dev/null ||
	fail 'Gateway digest rejection must be explicit'

printf '%s\n' 'standalone AX6S package tests passed'
