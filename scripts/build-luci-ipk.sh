#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd)
version=${1:-2.0.0-1}
dependency_mode=${2:-production}
package=luci-app-wificalling-location-gateway
source_dir="$root/openwrt/$package/files"
out_dir="$root/dist"
architecture=all
output_package=$package
description='Integrated WiFi Calling Gateway and WLOC LuCI service.'
target=all
if [ "$dependency_mode" = ax6s-standalone ]; then
	architecture=aarch64_cortex-a53
	output_package=wificalling-location-gateway
	description='Integrated WiFi Calling Gateway and WLOC service with unified LuCI.'
	target=mediatek/mt7622
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
		depends='luci-base, rpcd-mod-rpcsys'
		;;
	ax6s-existing|ax6s-full|ax6s-standalone)
		cp -R "$source_dir/." "$stage/data/"
		# AX6S keeps the in-repository Gateway module. The package is independent
		# of the external Gateway 1.7 repository, but Gateway/WLOC are one product.
		depends='luci-base, rpcd-mod-rpcsys, nftables, firewall4, kmod-nft-tproxy, kmod-nft-socket, ip-full'
		provides='wloc-service, wificalling-gateway'
		replaces='luci-app-wificalling-location-gateway, wloc-service'
		view_suffix=$(printf '%s' "$version" | tr '.-' '__')
		basic_name="wloc_basic_fix_$view_suffix"
		overview_name="wloc_overview_fix_$view_suffix"
		monitor_name="wloc_monitor_fix_$view_suffix"
		status_logs_name="gateway_status_logs_fix_$view_suffix"
		faq_name="wloc_faq_fix_$view_suffix"
		update_name="wloc_update_fix_$view_suffix"
		health_name="wloc_health_fix_$view_suffix"
		# Versioned view names bust the browser's resource cache: the LuCI
		# menu loads a fresh URL per package version, so an updated settings,
		# monitor, FAQ, or overview page is picked up without a manual cache
		# clear.
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/wloc-basic.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$basic_name.js"
		cp "$stage/data/www/luci-static/resources/view/wificalling-gateway/status-logs.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-gateway/$status_logs_name.js"
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/wloc-overview.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$overview_name.js"
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/wloc-monitor.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$monitor_name.js"
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/faq.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$faq_name.js"
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/wloc-update.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$update_name.js"
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/wloc-health.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$health_name.js"
		python3 - "$stage/data/usr/share/luci/menu.d/luci-app-wificalling-location-gateway.json" "$basic_name" "$overview_name" "$monitor_name" "$status_logs_name" "$faq_name" "$update_name" "$health_name" <<'PY'
import json
import sys

path = sys.argv[1]
basic_name = sys.argv[2]
overview_name = sys.argv[3]
monitor_name = sys.argv[4]
status_logs_name = sys.argv[5]
faq_name = sys.argv[6]
update_name = sys.argv[7]
health_name = sys.argv[8]
with open(path, encoding="utf-8") as handle:
    menu = json.load(handle)
menu["admin/services/wificalling-location-gateway/basic"]["action"]["path"] = (
    f"wificalling-location-gateway/{basic_name}"
)
menu["admin/services/wificalling-location-gateway/gateway-status-logs"]["action"]["path"] = (
    f"wificalling-gateway/{status_logs_name}"
)
menu["admin/services/wificalling-location-gateway/monitor"]["action"]["path"] = (
    f"wificalling-location-gateway/{monitor_name}"
)
menu["admin/services/wificalling-location-gateway/faq"]["action"]["path"] = (
    f"wificalling-location-gateway/{faq_name}"
)
menu["admin/services/wificalling-location-gateway/update"]["action"]["path"] = (
    f"wificalling-location-gateway/{update_name}"
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
			cp "$root/openwrt/files/usr/libexec/wificalling-location-gateway/singbox-runtime.sh" \
				"$stage/data/usr/libexec/wificalling-location-gateway/singbox-runtime.sh"
			cp "$service_bin" "$stage/data/usr/sbin/wloc-service"
			cp "$ctl_bin" "$stage/data/usr/sbin/wloc-ctl"
			chmod 0755 "$stage/data/etc/init.d/wloc-service" "$stage/data/usr/sbin/"*
			printf '%s\n' '/etc/config/wloc-service' '/etc/config/wificalling-gateway' > "$stage/control/conffiles"
			cat > "$stage/control/postinst" <<'POSTINST'
#!/bin/sh
[ -n "${IPKG_INSTROOT:-}" ] && exit 0
# A direct opkg install is outside the transactional updater. Do not leave a
# previous transaction result visible as if it described this package.
update_state=/var/lib/wificalling-location-gateway/update
rm -f "$update_state/status.json"
opkg_bin=$(command -v opkg 2>/dev/null || true)
if [ -n "$opkg_bin" ]; then
  installed_version=$("$opkg_bin" status wificalling-location-gateway 2>/dev/null | sed -n 's/^Version:[[:space:]]*//p' | head -n 1)
  recorded_version=$(cat "$update_state/current.version" 2>/dev/null || true)
  if [ -n "$installed_version" ] && [ "$installed_version" != "$recorded_version" ]; then
    # A direct install may have replaced the package behind the updater's
    # back. Never retain an older (possibly WLOC-only) rollback IPK.
    rm -f "$update_state/current.ipk" "$update_state/current.version"
  fi
fi
/etc/init.d/wloc-service disable >/dev/null 2>&1 || true
/etc/init.d/wloc-service stop >/dev/null 2>&1 || true
/etc/init.d/wificalling-gateway disable >/dev/null 2>&1 || true
/etc/init.d/wificalling-gateway stop >/dev/null 2>&1 || true
/etc/init.d/wificalling-location-gateway enable >/dev/null 2>&1 || true
if [ -x /usr/libexec/wificalling-location-gateway/singbox-runtime.sh ]; then
  /usr/libexec/wificalling-location-gateway/singbox-runtime.sh path >/dev/null 2>&1 || echo "wificalling-location-gateway: install sing-box tiny/lite or a PassWall sing-box provider" >&2
fi
/etc/init.d/wificalling-location-gateway restart >/dev/null 2>&1 || true
rm -f /tmp/luci-indexcache.*
/etc/init.d/rpcd reload >/dev/null 2>&1 || true
exit 0
POSTINST
			chmod 0755 "$stage/control/postinst"
		fi
		if [ "$dependency_mode" = ax6s-existing ]; then
			# The legacy AX6S mode reuses an already-installed WLOC binary,
			# while exposing the integrated Gateway/WLOC lifecycle helpers.
			mkdir -p "$stage/data/usr/sbin" "$stage/data/etc/init.d" \
				"$stage/data/usr/libexec/wificalling-location-gateway"
			for helper in wloc-health.sh wloc-support-bundle.sh wloc-component-update.sh \
				wloc-redirect-sync.sh wloc-profile-redirect.sh wloc-profile-status.sh; do
				cp "$root/openwrt/files/usr/sbin/$helper" "$stage/data/usr/sbin/$helper"
			done
			cp "$root/openwrt/files/etc/init.d/wificalling-location-gateway" \
				"$stage/data/etc/init.d/wificalling-location-gateway"
			cp "$root/openwrt/files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh" \
				"$stage/data/usr/libexec/wificalling-location-gateway/unified-supervisor.sh"
			cp "$root/openwrt/files/usr/libexec/wificalling-location-gateway/singbox-runtime.sh" \
				"$stage/data/usr/libexec/wificalling-location-gateway/singbox-runtime.sh"
			chmod 0755 "$stage/data/etc/init.d/wificalling-location-gateway" \
				"$stage/data/usr/sbin/"*.sh \
				"$stage/data/usr/libexec/wificalling-location-gateway/"*.sh
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
	'X-WLOC-Product: wificalling-location-gateway/v2' \
	'X-WLOC-Api: wloc.service/v2' \
	'X-WLOC-OpenWrt: 24.10+' \
	"X-WLOC-Target: $target" \
	"X-WLOC-Package-Format: ipk" \
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
"$root/scripts/ci/verify-package-budget.sh" "$out" >/dev/null
"$root/scripts/create-update-manifest.sh" "$out" >/dev/null

echo "$out"
