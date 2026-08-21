#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd)
version=${1:-1.2.0-1}
dependency_mode=${2:-production}
package=luci-app-wificalling-location-gateway
source_dir="$root/openwrt/$package/files"
out_dir="$root/dist"
architecture=all
output_package=$package
description='Unified LuCI UI for Wi-Fi Calling and WLOC location controls.'
if [ "$dependency_mode" = ax6s-standalone ]; then
	architecture=aarch64_cortex-a53
	output_package=wificalling-location-gateway
	description='Complete Wi-Fi Calling Gateway 1.7 and WLOC service with unified LuCI.'
fi
out="$out_dir/${output_package}_${version}_${architecture}.ipk"
stage=$(mktemp -d "${TMPDIR:-/tmp}/wloc-luci-ipk.XXXXXX")
trap 'rm -rf "$stage"' EXIT HUP INT TERM

tar_format=gnutar
case "$(tar --version 2>/dev/null | head -n 1)" in
	*GNU*) tar_format=gnu ;;
esac

make_archive() {
	archive_dir=$1
	archive_path=$2
	shift 2
	if [ "$tar_format" = gnu ]; then
		(cd "$archive_dir" && COPYFILE_DISABLE=1 tar --format "$tar_format" \
			--owner=0 --group=0 -czf "$archive_path" "$@")
	else
		(cd "$archive_dir" && COPYFILE_DISABLE=1 tar --format "$tar_format" \
			--uid 0 --gid 0 --uname root --gname root -czf "$archive_path" "$@")
	fi
}

mkdir -p "$stage/control" "$stage/data" "$out_dir"
cp -R "$source_dir/." "$stage/data/"
provides=
replaces=

archive_is_safe() {
	archive=$1
	tar -tzf "$archive" | while IFS= read -r member; do
		case "$member" in
			/*|../*|*/../*|*/..) exit 1 ;;
		esac
	done
}

sha256_file() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1" | awk '{print $1}'
	else
		shasum -a 256 "$1" | awk '{print $1}'
	fi
}

case "$dependency_mode" in
	production)
		depends='wloc-service, luci-app-wificalling-gateway, luci-base, rpcd-mod-rpcsys'
		;;
	ax6s-existing|ax6s-full|ax6s-standalone)
		# The validated AX6S predates package registration for its already-running
		# wloc-service. The existing/full variants retain the external Gateway
		# package dependency; standalone safely merges a pinned Gateway IPK.
		if [ "$dependency_mode" = ax6s-standalone ]; then
			gateway_ipk=${GATEWAY_IPK:-}
			gateway_sha=${GATEWAY_IPK_SHA256:-}
			[ -f "$gateway_ipk" ] || { echo "missing Gateway IPK: $gateway_ipk" >&2; exit 2; }
			[ -n "$gateway_sha" ] || { echo 'GATEWAY_IPK_SHA256 is required' >&2; exit 2; }
			actual_gateway_sha=$(sha256_file "$gateway_ipk")
			[ "$actual_gateway_sha" = "$gateway_sha" ] || {
				echo "Gateway IPK SHA-256 mismatch: expected $gateway_sha, got $actual_gateway_sha" >&2
				exit 2
			}
			gateway_stage="$stage/gateway"
			mkdir -p "$gateway_stage/package" "$gateway_stage/data"
			tar -xf "$gateway_ipk" -C "$gateway_stage/package"
			gateway_control=$(tar -xOf "$gateway_stage/package/control.tar.gz" ./control)
			printf '%s\n' "$gateway_control" | grep -Fx 'Package: luci-app-wificalling-gateway' >/dev/null || {
				echo 'Gateway IPK has an unexpected package identity' >&2
				exit 2
			}
			printf '%s\n' "$gateway_control" | grep -E '^Version: 1\.7\.[0-9]+-[0-9]+$' >/dev/null || {
				echo 'Gateway IPK must be a validated 1.7.x release' >&2
				exit 2
			}
			archive_is_safe "$gateway_stage/package/data.tar.gz" || {
				echo 'Gateway IPK contains an unsafe path' >&2
				exit 2
			}
			tar -xzf "$gateway_stage/package/data.tar.gz" -C "$gateway_stage/data"
			# The Gateway 1.7.x compiler has no WireGuard pre-shared key
			# support; the patch adds it (fail-closed against future
			# Gateway versions).
			"$root/scripts/openwrt/patch-wireguard-psk.sh" "$gateway_stage/data"
			"$root/scripts/openwrt/patch-wireguard-health.sh" "$gateway_stage/data"
			"$root/scripts/openwrt/patch-node-status-compact.sh" "$gateway_stage/data"
			"$root/scripts/openwrt/patch-gateway-device-guard.sh" "$gateway_stage/data"
			cp -R "$gateway_stage/data/." "$stage/data/"
			# The integrated LuCI views intentionally replace the standalone
			# Gateway views after the verified Gateway payload is merged.
			cp -R "$source_dir/." "$stage/data/"
			rm -f "$stage/data/usr/share/luci/menu.d/luci-app-wificalling-gateway.json"
			depends='luci-base, rpcd-mod-rpcsys, sing-box, nftables, firewall4, kmod-nft-tproxy, kmod-nft-socket, ip-full'
			provides='luci-app-wificalling-location-gateway, luci-app-wificalling-gateway, wloc-service'
			replaces='luci-app-wificalling-location-gateway, luci-app-wificalling-gateway, wloc-service'
		else
			depends='luci-app-wificalling-gateway, luci-base, rpcd-mod-rpcsys'
		fi
		view_suffix=$(printf '%s' "$version" | tr '.-' '__')
		view_name="wloc_mode_fix_$view_suffix"
		monitor_name="wloc_monitor_fix_$view_suffix"
		faq_name="wloc_faq_fix_$view_suffix"
		wfc_name="wfc_overview_fix_$view_suffix"
		health_name="wloc_health_fix_$view_suffix"
		# Versioned view names bust the browser's resource cache: the LuCI
		# menu loads a fresh URL per package version, so an updated settings,
		# monitor, FAQ, or overview page is picked up without a manual cache
		# clear.
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/wloc.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$view_name.js"
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/wloc-monitor.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$monitor_name.js"
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/faq.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$faq_name.js"
			cp "$stage/data/www/luci-static/resources/view/wificalling-gateway/overview.js" \
				"$stage/data/www/luci-static/resources/view/wificalling-gateway/$wfc_name.js"
		# The external Gateway owns the unversioned view; retain the copied,
		# versioned integrated view so this package does not duplicate its menu.
		if [ "$dependency_mode" != ax6s-standalone ]; then
			rm -f "$stage/data/www/luci-static/resources/view/wificalling-gateway/overview.js"
		fi
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/wloc-health.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$health_name.js"
		python3 - "$stage/data/usr/share/luci/menu.d/luci-app-wificalling-location-gateway.json" "$view_name" "$monitor_name" "$faq_name" "$wfc_name" "$health_name" <<'PY'
import json
import sys

path = sys.argv[1]
view_name = sys.argv[2]
monitor_name = sys.argv[3]
faq_name = sys.argv[4]
wfc_name = sys.argv[5]
health_name = sys.argv[6]
with open(path, encoding="utf-8") as handle:
    menu = json.load(handle)
menu["admin/services/wificalling-location-gateway/wloc"]["action"]["path"] = (
    f"wificalling-location-gateway/{view_name}"
)
menu["admin/services/wificalling-location-gateway/wloc-monitor"]["action"]["path"] = (
    f"wificalling-location-gateway/{monitor_name}"
)
menu["admin/services/wificalling-location-gateway/faq"]["action"]["path"] = (
    f"wificalling-location-gateway/{faq_name}"
)
menu["admin/services/wificalling-location-gateway/wfc"]["action"]["path"] = (
    f"wificalling-gateway/{wfc_name}"
)
menu["admin/services/wificalling-location-gateway/health"]["action"]["path"] = (
    f"wificalling-location-gateway/{health_name}"
)
with open(path, "w", encoding="utf-8") as handle:
    json.dump(menu, handle, ensure_ascii=False, indent=2)
    handle.write("\n")
PY
		if [ "$dependency_mode" = ax6s-full ] || [ "$dependency_mode" = ax6s-standalone ]; then
			service_bin=${WLOC_SERVICE_BIN:-$out_dir/wloc-service_aarch64-openwrt-linux-musl}
			ctl_bin=${WLOC_CTL_BIN:-$out_dir/wloc-ctl_aarch64-openwrt-linux-musl}
			[ -x "$service_bin" ] || { echo "missing WLOC service binary: $service_bin" >&2; exit 2; }
			[ -x "$ctl_bin" ] || { echo "missing WLOC control binary: $ctl_bin" >&2; exit 2; }
			mkdir -p "$stage/data/etc/config" "$stage/data/etc/init.d" "$stage/data/usr/sbin"
			cp "$root/openwrt/files/etc/config/wloc-service" "$stage/data/etc/config/wloc-service"
			cp "$root/openwrt/files/etc/init.d/wloc-service" "$stage/data/etc/init.d/wloc-service"
			cp "$root/openwrt/files/usr/sbin/wloc-redirect-sync.sh" "$stage/data/usr/sbin/wloc-redirect-sync.sh"
			cp "$root/openwrt/files/usr/sbin/wloc-refresh-set.sh" "$stage/data/usr/sbin/wloc-refresh-set.sh"
			cp "$root/openwrt/files/usr/sbin/wloc-profile-redirect.sh" "$stage/data/usr/sbin/wloc-profile-redirect.sh"
			cp "$root/openwrt/files/usr/sbin/wloc-profile-status.sh" "$stage/data/usr/sbin/wloc-profile-status.sh"
			cp "$root/openwrt/files/usr/sbin/wloc-health.sh" "$stage/data/usr/sbin/wloc-health.sh"
			cp "$root/openwrt/files/usr/sbin/wloc-support-bundle.sh" "$stage/data/usr/sbin/wloc-support-bundle.sh"
			cp "$root/openwrt/files/usr/sbin/wloc-component-update.sh" "$stage/data/usr/sbin/wloc-component-update.sh"
			mkdir -p "$stage/data/etc/init.d" "$stage/data/usr/libexec/wificalling-location-gateway"
			cp "$root/openwrt/files/etc/init.d/wificalling-location-gateway" "$stage/data/etc/init.d/wificalling-location-gateway"
			cp "$root/openwrt/files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh" \
				"$stage/data/usr/libexec/wificalling-location-gateway/unified-supervisor.sh"
			cp "$service_bin" "$stage/data/usr/sbin/wloc-service"
			cp "$ctl_bin" "$stage/data/usr/sbin/wloc-ctl"
			chmod 0755 "$stage/data/etc/init.d/wloc-service" "$stage/data/usr/sbin/"*
			if [ "$dependency_mode" = ax6s-standalone ]; then
				printf '%s\n' \
					'/etc/config/wificalling-gateway' \
					'/etc/config/wloc-service' > "$stage/control/conffiles"
			else
				printf '%s\n' '/etc/config/wloc-service' > "$stage/control/conffiles"
			fi
			cat > "$stage/control/postinst" <<'POSTINST'
#!/bin/sh
[ -n "${IPKG_INSTROOT:-}" ] && exit 0
/etc/init.d/wificalling-gateway disable >/dev/null 2>&1 || true
/etc/init.d/wloc-service disable >/dev/null 2>&1 || true
/etc/init.d/wificalling-location-gateway enable >/dev/null 2>&1 || true
killall -q wloc-service >/dev/null 2>&1 || true
rm -f /var/run/wloc-service/control.sock
mkdir -p /var/run/wificalling-gateway
chmod 0700 /var/run/wificalling-gateway
/etc/init.d/wificalling-location-gateway restart >/dev/null 2>&1 || true
rm -f /tmp/luci-indexcache.*
/etc/init.d/rpcd reload >/dev/null 2>&1 || true
exit 0
POSTINST
			chmod 0755 "$stage/control/postinst"
		fi
		if [ "$dependency_mode" = ax6s-existing ]; then
			# The legacy AX6S mode reuses the already-installed Gateway and
			# wloc-service binaries, but it still needs the same V2 lifecycle
			# boundary and update/health helpers exposed by the LuCI page.
			mkdir -p "$stage/data/usr/sbin" "$stage/data/etc/init.d" \
				"$stage/data/usr/libexec/wificalling-location-gateway"
			for helper in wloc-health.sh wloc-component-update.sh \
				wloc-redirect-sync.sh wloc-profile-redirect.sh wloc-profile-status.sh; do
				cp "$root/openwrt/files/usr/sbin/$helper" "$stage/data/usr/sbin/$helper"
			done
			cp "$root/openwrt/files/etc/init.d/wificalling-location-gateway" \
				"$stage/data/etc/init.d/wificalling-location-gateway"
			cp "$root/openwrt/files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh" \
				"$stage/data/usr/libexec/wificalling-location-gateway/unified-supervisor.sh"
			chmod 0755 "$stage/data/etc/init.d/wificalling-location-gateway" \
				"$stage/data/usr/sbin/"*.sh \
				"$stage/data/usr/libexec/wificalling-location-gateway/unified-supervisor.sh"
		fi
		;;
	*)
	echo "unsupported dependency mode: $dependency_mode" >&2
	exit 2
	;;
esac

printf '%s\n' \
	"Package: $output_package" \
	"Version: $version" \
	"Architecture: $architecture" \
	'Maintainer: wificalling-location-gateway maintainers' \
	"Depends: $depends" \
	'X-WFC-Product: wificalling-location-gateway/v2' \
	'X-WFC-Gateway: 1.7' \
	'X-WFC-Wloc-Api: wloc.service/v2' \
	'Section: luci' \
	'Priority: optional' \
	'License: MIT' \
	"Description: $description" \
	> "$stage/control/control"
[ -z "$provides" ] || printf 'Provides: %s\n' "$provides" >> "$stage/control/control"
[ -z "$replaces" ] || printf 'Replaces: %s\n' "$replaces" >> "$stage/control/control"
printf '2.0\n' > "$stage/debian-binary"

make_archive "$stage/control" "$stage/control.tar.gz" .
make_archive "$stage/data" "$stage/data.tar.gz" .
rm -f "$out"
make_archive "$stage" "$out" debian-binary data.tar.gz control.tar.gz
"$root/scripts/create-update-manifest.sh" "$out" >/dev/null

echo "$out"
