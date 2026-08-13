#!/bin/sh
set -eu

root=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd)
version=${1:-0.1.0-2}
dependency_mode=${2:-production}
package=luci-app-wificalling-location-gateway
source_dir="$root/openwrt/$package/files"
out_dir="$root/dist"
out="$out_dir/${package}_${version}_all.ipk"
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

case "$dependency_mode" in
	production)
		depends='wloc-service, luci-app-wificalling-gateway, luci-base, rpcd-mod-rpcsys'
		;;
	ax6s-existing|ax6s-full)
		# The validated AX6S predates package registration for its already-running
		# wloc-service. This variant only installs LuCI files and keeps runtime
		# service/config ownership outside opkg.
		depends='luci-app-wificalling-gateway, luci-base, rpcd-mod-rpcsys'
		rm -f "$stage/data/www/luci-static/resources/view/wificalling-gateway/overview.js"
		view_suffix=$(printf '%s' "$version" | tr '.-' '__')
		view_name="wloc_mode_fix_$view_suffix"
		cp "$stage/data/www/luci-static/resources/view/wificalling-location-gateway/wloc.js" \
			"$stage/data/www/luci-static/resources/view/wificalling-location-gateway/$view_name.js"
		python3 - "$stage/data/usr/share/luci/menu.d/luci-app-wificalling-location-gateway.json" "$view_name" <<'PY'
import json
import sys

path = sys.argv[1]
view_name = sys.argv[2]
with open(path, encoding="utf-8") as handle:
    menu = json.load(handle)
menu["admin/services/wificalling-location-gateway/wloc"]["action"]["path"] = (
    f"wificalling-location-gateway/{view_name}"
)
with open(path, "w", encoding="utf-8") as handle:
    json.dump(menu, handle, ensure_ascii=False, indent=2)
    handle.write("\n")
PY
		if [ "$dependency_mode" = ax6s-full ]; then
			service_bin=${WLOC_SERVICE_BIN:-$out_dir/wloc-service_aarch64-openwrt-linux-musl}
			ctl_bin=${WLOC_CTL_BIN:-$out_dir/wloc-ctl_aarch64-openwrt-linux-musl}
			[ -x "$service_bin" ] || { echo "missing WLOC service binary: $service_bin" >&2; exit 2; }
			[ -x "$ctl_bin" ] || { echo "missing WLOC control binary: $ctl_bin" >&2; exit 2; }
			mkdir -p "$stage/data/etc/config" "$stage/data/etc/init.d" "$stage/data/usr/sbin"
			cp "$root/openwrt/files/etc/config/wloc-service" "$stage/data/etc/config/wloc-service"
			cp "$root/openwrt/files/etc/init.d/wloc-service" "$stage/data/etc/init.d/wloc-service"
			cp "$root/openwrt/files/usr/sbin/wloc-redirect-sync.sh" "$stage/data/usr/sbin/wloc-redirect-sync.sh"
			cp "$root/openwrt/files/usr/sbin/wloc-refresh-set.sh" "$stage/data/usr/sbin/wloc-refresh-set.sh"
			cp "$service_bin" "$stage/data/usr/sbin/wloc-service"
			cp "$ctl_bin" "$stage/data/usr/sbin/wloc-ctl"
			chmod 0755 "$stage/data/etc/init.d/wloc-service" "$stage/data/usr/sbin/"*
			printf '%s\n' '/etc/config/wloc-service' > "$stage/control/conffiles"
			cat > "$stage/control/postinst" <<'POSTINST'
#!/bin/sh
[ -n "${IPKG_INSTROOT:-}" ] && exit 0
/etc/init.d/wloc-service enable >/dev/null 2>&1 || true
killall -q wloc-service >/dev/null 2>&1 || true
rm -f /var/run/wloc-service/control.sock
/etc/init.d/wloc-service restart >/dev/null 2>&1 || true
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
	"Package: $package" \
	"Version: $version" \
	'Architecture: all' \
	'Maintainer: wificalling-location-gateway maintainers' \
	"Depends: $depends" \
	'Section: luci' \
	'Priority: optional' \
	'License: MIT' \
	'Description: Unified LuCI UI for Wi-Fi Calling and WLOC location controls.' \
	> "$stage/control/control"
printf '2.0\n' > "$stage/debian-binary"

make_archive "$stage/control" "$stage/control.tar.gz" .
make_archive "$stage/data" "$stage/data.tar.gz" .
rm -f "$out"
make_archive "$stage" "$out" debian-binary data.tar.gz control.tar.gz

echo "$out"
