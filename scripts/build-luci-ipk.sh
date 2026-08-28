#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd)
version=${1:-1.3.0-r8}
dependency_mode=${2:-production}
package=luci-app-wificalling-location-gateway
source_dir="$root/openwrt/$package/files"
out_dir="$root/dist"
architecture=all
output_package=$package
description='Unified LuCI UI for Wi-Fi Calling and WLOC location controls.'
license=MIT
variant=standard
if [ "$dependency_mode" = ax6s-lite ]; then
	variant=lite
	license='MIT, GPL-3.0-or-later'
	architecture=aarch64_cortex-a53
	output_package=wificalling-location-gateway-lite
	description='Complete Wi-Fi Calling Gateway and WLOC service with bundled sing-box Lite.'
elif [ "$dependency_mode" = ax6s-standard ] || [ "$dependency_mode" = ax6s-standalone ]; then
	architecture=aarch64_cortex-a53
	output_package=wificalling-location-gateway
	if [ "$dependency_mode" = ax6s-standard ]; then
		description='Complete Wi-Fi Calling Gateway and WLOC service using the system sing-box.'
	else
		description='Complete Wi-Fi Calling Gateway and WLOC service with unified LuCI.'
	fi
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
conflicts=

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
	ax6s-existing|ax6s-full|ax6s-standalone|ax6s-standard|ax6s-lite)
		# The validated AX6S predates package registration for its already-running
		# wloc-service. The existing/full variants retain the external Gateway
		# package dependency; standalone safely merges a pinned Gateway IPK.
		if [ "$dependency_mode" = ax6s-standalone ] || [ "$dependency_mode" = ax6s-standard ] || [ "$dependency_mode" = ax6s-lite ]; then
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
			if ! printf '%s\n' "$gateway_control" | grep -Fx 'Package: wificalling-location-gateway' >/dev/null; then
				echo 'Gateway IPK must be the stable integrated 1.3.0-r1 release' >&2
				exit 2
			fi
			printf '%s\n' "$gateway_control" | grep -Fx 'Version: 1.3.0-r1' >/dev/null || {
				echo 'Gateway IPK must be the stable integrated 1.3.0-r1 release' >&2
				exit 2
			}
			archive_is_safe "$gateway_stage/package/data.tar.gz" || {
				echo 'Gateway IPK contains an unsafe path' >&2
				exit 2
			}
			tar -xzf "$gateway_stage/package/data.tar.gz" -C "$gateway_stage/data"
			# Overlay the current maintained baseline after extraction so an
			# incremental upgrade cannot resurrect older scripts or LuCI assets.
			cp -R "$root/openwrt/files/." "$gateway_stage/data/"
			cp -R "$gateway_stage/data/." "$stage/data/"
			# The integrated LuCI views intentionally replace the standalone
			# Gateway views after the verified Gateway payload is merged.
			cp -R "$source_dir/." "$stage/data/"
			rm -f "$stage/data/usr/share/luci/menu.d/luci-app-wificalling-gateway.json"
			if [ "$variant" = lite ]; then
				depends='luci-base, rpcd-mod-rpcsys, ca-bundle, kmod-inet-diag, kmod-netlink-diag, kmod-tun, nftables, firewall4, kmod-nft-tproxy, kmod-nft-socket, ip-full'
				provides='wificalling-location-gateway, sing-box, luci-app-wificalling-location-gateway, luci-app-wificalling-gateway, wloc-service'
				replaces='wificalling-location-gateway, sing-box, luci-app-wificalling-location-gateway, luci-app-wificalling-gateway, wloc-service'
				conflicts='wificalling-location-gateway, sing-box'
			else
				depends='luci-base, rpcd-mod-rpcsys, sing-box, nftables, firewall4, kmod-nft-tproxy, kmod-nft-socket, ip-full'
				provides='luci-app-wificalling-location-gateway, luci-app-wificalling-gateway, wloc-service'
				replaces='luci-app-wificalling-location-gateway, luci-app-wificalling-gateway, wloc-service'
				conflicts='wificalling-location-gateway-lite'
			fi
		else
			depends='luci-app-wificalling-gateway, luci-base, rpcd-mod-rpcsys'
			rm -f "$stage/data/www/luci-static/resources/view/wificalling-gateway/overview.js"
		fi
		view_suffix=$(printf '%s' "$version" | tr '.-' '__')
		view_name="wloc_mode_fix_$view_suffix"
		monitor_name="wloc_monitor_fix_$view_suffix"
		faq_name="wloc_faq_fix_$view_suffix"
		wfc_name="wfc_overview_fix_$view_suffix"
		import_name="node-import_fix_$view_suffix"
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
		cp "$stage/data/www/luci-static/resources/wificalling-gateway/node-import.js" \
			"$stage/data/www/luci-static/resources/wificalling-gateway/$import_name.js"
		sed "s/wificalling-gateway\\.node-import/wificalling-gateway.$import_name/" \
			"$stage/data/www/luci-static/resources/view/wificalling-gateway/overview.js" \
			> "$stage/data/www/luci-static/resources/view/wificalling-gateway/$wfc_name.js"
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
		if [ "$dependency_mode" = ax6s-full ] || [ "$dependency_mode" = ax6s-standalone ] || [ "$dependency_mode" = ax6s-standard ] || [ "$dependency_mode" = ax6s-lite ]; then
			service_bin=${WLOC_SERVICE_BIN:-$out_dir/wloc-service_aarch64-openwrt-linux-musl}
			ctl_bin=${WLOC_CTL_BIN:-$out_dir/wloc-ctl_aarch64-openwrt-linux-musl}
			[ -x "$service_bin" ] || { echo "missing WLOC service binary: $service_bin" >&2; exit 2; }
			[ -x "$ctl_bin" ] || { echo "missing WLOC control binary: $ctl_bin" >&2; exit 2; }
			mkdir -p "$stage/data/etc/config" "$stage/data/etc/init.d" "$stage/data/usr/sbin"
			cp "$root/openwrt/files/etc/config/wloc-service" "$stage/data/etc/config/wloc-service"
			cp "$root/openwrt/files/etc/init.d/wloc-service" "$stage/data/etc/init.d/wloc-service"
			cp "$root/openwrt/files/usr/sbin/wloc-redirect-sync.sh" "$stage/data/usr/sbin/wloc-redirect-sync.sh"
			cp "$root/openwrt/files/usr/sbin/wloc-refresh-set.sh" "$stage/data/usr/sbin/wloc-refresh-set.sh"
			cp "$root/openwrt/files/usr/sbin/wloc-health.sh" "$stage/data/usr/sbin/wloc-health.sh"
			cp "$service_bin" "$stage/data/usr/sbin/wloc-service"
			cp "$ctl_bin" "$stage/data/usr/sbin/wloc-ctl"
			chmod 0755 "$stage/data/etc/init.d/wloc-service" "$stage/data/usr/sbin/"*
			mkdir -p "$stage/data/usr/share/wificalling-location-gateway"
			printf '%s\n' "$variant" > "$stage/data/usr/share/wificalling-location-gateway/runtime-variant"
			if [ "$variant" = lite ]; then
				tiny_bin=${SINGBOX_LITE_BIN:-}
				tiny_sha=${SINGBOX_LITE_SHA256:-}
				[ -x "$tiny_bin" ] || { echo "missing sing-box Lite binary: $tiny_bin" >&2; exit 2; }
				[ -n "$tiny_sha" ] || { echo 'SINGBOX_LITE_SHA256 is required' >&2; exit 2; }
				actual_tiny_sha=$(sha256_file "$tiny_bin")
				[ "$actual_tiny_sha" = "$tiny_sha" ] || {
					echo "sing-box Lite SHA-256 mismatch: expected $tiny_sha, got $actual_tiny_sha" >&2
					exit 2
				}
				case "$(file "$tiny_bin" 2>/dev/null)" in
					*ELF*ARM\ aarch64*) ;;
					*) echo 'sing-box Lite binary must be an AArch64 ELF' >&2; exit 2 ;;
				esac
				"$root/scripts/openwrt/package-singbox-lite.sh" \
					"$stage/data" "$tiny_bin" "$tiny_sha"
			fi
			if [ "$dependency_mode" = ax6s-standalone ] || [ "$dependency_mode" = ax6s-standard ] || [ "$dependency_mode" = ax6s-lite ]; then
				printf '%s\n' \
					'/etc/config/wificalling-gateway' \
					'/etc/config/wloc-service' > "$stage/control/conffiles"
			else
				printf '%s\n' '/etc/config/wloc-service' > "$stage/control/conffiles"
			fi
			cat > "$stage/control/preinst" <<'PREINST'
#!/bin/sh
[ -n "${IPKG_INSTROOT:-}" ] && exit 0
/etc/init.d/wloc-service stop >/dev/null 2>&1 || true
/etc/init.d/wificalling-gateway stop >/dev/null 2>&1 || true
rm -rf /tmp/wloc-probe
exit 0
PREINST
			cp "$stage/control/preinst" "$stage/control/prerm"
			chmod 0755 "$stage/control/preinst" "$stage/control/prerm"
			cat > "$stage/control/postinst" <<'POSTINST'
#!/bin/sh
[ -n "${IPKG_INSTROOT:-}" ] && exit 0
/etc/init.d/wificalling-gateway enable >/dev/null 2>&1 || true
/etc/init.d/wloc-service enable >/dev/null 2>&1 || true
killall -q wloc-service >/dev/null 2>&1 || true
rm -f /var/run/wloc-service/control.sock
mkdir -p /var/run/wificalling-gateway
chmod 0700 /var/run/wificalling-gateway
/etc/init.d/wificalling-gateway restart >/dev/null 2>&1 || true
/etc/init.d/wloc-service restart >/dev/null 2>&1 || true
rm -f /tmp/luci-indexcache.*
/etc/init.d/rpcd reload >/dev/null 2>&1 || true
exit 0
POSTINST
			chmod 0755 "$stage/control/postinst"
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
	'Section: luci' \
	'Priority: optional' \
	"License: $license" \
	"Description: $description" \
	> "$stage/control/control"
[ -z "$provides" ] || printf 'Provides: %s\n' "$provides" >> "$stage/control/control"
[ -z "$replaces" ] || printf 'Replaces: %s\n' "$replaces" >> "$stage/control/control"
[ -z "$conflicts" ] || printf 'Conflicts: %s\n' "$conflicts" >> "$stage/control/control"
printf '2.0\n' > "$stage/debian-binary"

make_archive "$stage/control" "$stage/control.tar.gz" .
make_archive "$stage/data" "$stage/data.tar.gz" .
rm -f "$out"
make_archive "$stage" "$out" debian-binary data.tar.gz control.tar.gz

echo "$out"
