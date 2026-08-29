#!/bin/sh
set -eu

OPENWRT_24_SDK='ghcr.io/openwrt/sdk:x86_64-24.10.8@sha256:b28d5e4087dbd3f815a8bf5440a11e54e6bbd3d7400c3729d872e7940a4a77c1'
OPENWRT_25_SDK='ghcr.io/openwrt/sdk:x86_64-25.12.3@sha256:a0ab488698b70d6585dc35bebb77b3f6d9523fd68873fab78a1bd19cc123cd0f'

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
version=1.3.0
release=13
arch=x86_64
service_bin=
ctl_bin=
gateway_ipk=
gateway_sha256=
out_dir="$repo_root/dist/openwrt-release"
plan_only=0
variants=standard
singbox_lite_bin=
singbox_lite_sha256=

fail() {
	printf 'build-release-packages: %s\n' "$*" >&2
	exit 2
}

usage() {
	cat <<'EOF'
Usage: build-release-packages.sh [--plan] [options]

Options:
  --version VERSION          Package version (default: 1.3.0)
  --release RELEASE          Package release number (default: 13)
  --arch ARCH                OpenWrt runtime architecture (default: x86_64)
  --service-bin PATH         Static wloc-service binary (required)
  --ctl-bin PATH             Static wloc-ctl binary (required)
  --gateway-ipk PATH         Pinned stable Gateway/integrated IPK (required to build)
  --gateway-sha256 SHA256    Expected base IPK digest (required to build)
  --out-dir PATH             Output directory
  --variants LIST            standard, lite, or standard,lite (default: standard)
  --singbox-lite-bin PATH    Architecture-matched sing-box Lite binary
  --singbox-lite-sha256 SHA  Expected Lite binary digest
  --plan                     Print the immutable build plan without Docker
EOF
}

while [ "$#" -gt 0 ]; do
	case "$1" in
		--version) [ "$#" -ge 2 ] || fail 'missing --version value'; version=$2; shift 2 ;;
		--release) [ "$#" -ge 2 ] || fail 'missing --release value'; release=$2; shift 2 ;;
		--arch) [ "$#" -ge 2 ] || fail 'missing --arch value'; arch=$2; shift 2 ;;
		--service-bin) [ "$#" -ge 2 ] || fail 'missing --service-bin value'; service_bin=$2; shift 2 ;;
		--ctl-bin) [ "$#" -ge 2 ] || fail 'missing --ctl-bin value'; ctl_bin=$2; shift 2 ;;
		--gateway-ipk) [ "$#" -ge 2 ] || fail 'missing --gateway-ipk value'; gateway_ipk=$2; shift 2 ;;
		--gateway-sha256) [ "$#" -ge 2 ] || fail 'missing --gateway-sha256 value'; gateway_sha256=$2; shift 2 ;;
		--out-dir) [ "$#" -ge 2 ] || fail 'missing --out-dir value'; out_dir=$2; shift 2 ;;
		--variants) [ "$#" -ge 2 ] || fail 'missing --variants value'; variants=$2; shift 2 ;;
		--singbox-lite-bin) [ "$#" -ge 2 ] || fail 'missing --singbox-lite-bin value'; singbox_lite_bin=$2; shift 2 ;;
		--singbox-lite-sha256) [ "$#" -ge 2 ] || fail 'missing --singbox-lite-sha256 value'; singbox_lite_sha256=$2; shift 2 ;;
		--plan) plan_only=1; shift ;;
		-h|--help) usage; exit 0 ;;
		*) fail "unknown argument: $1" ;;
	esac
done

case "$version" in ''|*[!0-9A-Za-z.+~-]*) fail 'invalid package version' ;; esac
case "$release" in ''|*[!0-9]*) fail 'release must be numeric' ;; esac
case "$arch" in
	all|noarch) fail 'runtime architecture must not be all or noarch' ;;
	''|*[!0-9A-Za-z_+-]*) fail 'invalid runtime architecture' ;;
esac
[ "$arch" = x86_64 ] || fail 'this SDK matrix currently supports x86_64 only'
[ -n "$service_bin" ] || fail '--service-bin is required'
[ -n "$ctl_bin" ] || fail '--ctl-bin is required'
[ -x "$service_bin" ] || fail "service binary is not executable: $service_bin"
[ -x "$ctl_bin" ] || fail "control binary is not executable: $ctl_bin"

case "$variants" in
	standard|lite|standard,lite) ;;
	*) fail 'variants must be standard, lite, or standard,lite' ;;
esac
case ",$variants," in
	*,lite,*)
		[ -n "$singbox_lite_bin" ] || fail '--singbox-lite-bin is required for the Lite variant'
		[ -x "$singbox_lite_bin" ] || fail "sing-box Lite binary is not executable: $singbox_lite_bin"
		[ -n "$singbox_lite_sha256" ] || fail '--singbox-lite-sha256 is required for the Lite variant'
		case "$singbox_lite_sha256" in *[!0-9a-fA-F]*|'') fail 'invalid sing-box Lite SHA-256' ;; esac
		[ "${#singbox_lite_sha256}" -eq 64 ] || fail 'invalid sing-box Lite SHA-256'
		actual_lite_sha=$(shasum -a 256 "$singbox_lite_bin" | awk '{print $1}')
		[ "$actual_lite_sha" = "$singbox_lite_sha256" ] || fail 'sing-box Lite SHA-256 mismatch'
		;;
esac

cat <<EOF
24.10 SDK: $OPENWRT_24_SDK
25.12 SDK: $OPENWRT_25_SDK
EOF
case ",$variants," in
	*,standard,*)
		printf '24.10 standard: wificalling-location-gateway_%s-r%s_%s.ipk\n' "$version" "$release" "$arch"
		printf '25.12 standard: wificalling-location-gateway-%s-r%s.apk (arch: %s)\n' "$version" "$release" "$arch"
		printf '%s\n' 'standard runtime: firmware /usr/bin/sing-box'
		;;
esac
case ",$variants," in
	*,lite,*)
		printf '24.10 Lite: wificalling-location-gateway-lite_%s-r%s_%s.ipk\n' "$version" "$release" "$arch"
		printf '25.12 Lite: wificalling-location-gateway-lite-%s-r%s.apk (arch: %s)\n' "$version" "$release" "$arch"
		printf 'lite runtime: %s\n' "$singbox_lite_sha256"
		;;
esac

[ "$plan_only" -eq 0 ] || exit 0
command -v docker >/dev/null 2>&1 || fail 'docker is required'
case "$out_dir" in /*) ;; *) fail '--out-dir must be absolute' ;; esac
case "${out_dir##*/}" in
	openwrt-release|wloc-openwrt-release-?*) ;;
	*) fail '--out-dir must be a dedicated openwrt-release directory' ;;
esac
case "$out_dir" in /|"$repo_root") fail 'unsafe --out-dir' ;; esac
[ ! -L "$out_dir" ] || fail '--out-dir must not be a symbolic link'
[ -f "$gateway_ipk" ] || fail '--gateway-ipk must name an existing file'
[ -n "$gateway_sha256" ] || fail '--gateway-sha256 is required'
case "$gateway_sha256" in *[!0-9a-fA-F]*|'') fail 'invalid Gateway SHA-256' ;; esac
[ "${#gateway_sha256}" -eq 64 ] || fail 'invalid Gateway SHA-256'
actual_gateway_sha=$(shasum -a 256 "$gateway_ipk" | awk '{print $1}')
[ "$actual_gateway_sha" = "$gateway_sha256" ] || fail 'Gateway IPK SHA-256 mismatch'

stage=$(mktemp -d "${TMPDIR:-/tmp}/wloc-openwrt-package.XXXXXX")
trap 'rm -rf "$stage"' EXIT HUP INT TERM
package_dir="$stage/input/wificalling-location-gateway"
mkdir -p "$package_dir/files" "$stage/gateway" "$stage/output"
chmod 0777 "$stage/output"

tar -xf "$gateway_ipk" -C "$stage/gateway"
gateway_control=$(tar -xOf "$stage/gateway/control.tar.gz" ./control)
printf '%s\n' "$gateway_control" | grep -Fx 'Package: wificalling-location-gateway' >/dev/null ||
	fail 'Gateway IPK must be the stable integrated 1.3.0-r1 release'
printf '%s\n' "$gateway_control" | grep -Fx 'Version: 1.3.0-r1' >/dev/null ||
	fail 'Gateway IPK must be the stable integrated 1.3.0-r1 release'
tar -tzf "$stage/gateway/data.tar.gz" | while IFS= read -r member; do
	case "$member" in /*|../*|*/../*|*/..) fail 'Gateway IPK contains an unsafe path' ;; esac
done
tar -xzf "$stage/gateway/data.tar.gz" -C "$package_dir/files"

# Overlay the current maintained baseline wholesale so an incremental release
# cannot regress Gateway/WLOC scripts, monitoring, configuration, or LuCI.
cp -R "$repo_root/openwrt/files/." "$package_dir/files/"

# Overlay the integrated UI, then the architecture-specific WLOC runtime.
cp -R "$repo_root/openwrt/luci-app-wificalling-location-gateway/files/." "$package_dir/files/"
rm -f "$package_dir/files/usr/share/luci/menu.d/luci-app-wificalling-gateway.json"
view_suffix=$(printf '%s-r%s' "$version" "$release" | tr '.-' '__')
import_name="node-import_fix_$view_suffix"
wfc_name="wfc_overview_fix_$view_suffix"
cp "$package_dir/files/www/luci-static/resources/wificalling-gateway/node-import.js" \
	"$package_dir/files/www/luci-static/resources/wificalling-gateway/$import_name.js"
sed "s/wificalling-gateway\\.node-import/wificalling-gateway.$import_name/" \
	"$package_dir/files/www/luci-static/resources/view/wificalling-gateway/overview.js" \
	> "$package_dir/files/www/luci-static/resources/view/wificalling-gateway/$wfc_name.js"
python3 - "$package_dir/files/usr/share/luci/menu.d/luci-app-wificalling-location-gateway.json" "$wfc_name" <<'PY'
import json
import sys

path, wfc_name = sys.argv[1:]
with open(path, encoding="utf-8") as handle:
	menu = json.load(handle)
menu["admin/services/wificalling-location-gateway/wfc"]["action"]["path"] = (
		f"wificalling-gateway/{wfc_name}"
)
with open(path, "w", encoding="utf-8") as handle:
	json.dump(menu, handle, ensure_ascii=False, indent=2)
	handle.write("\n")
PY
mkdir -p "$package_dir/files/usr/sbin" "$package_dir/files/etc/init.d" "$package_dir/files/etc/config"
cp "$service_bin" "$package_dir/files/usr/sbin/wloc-service"
cp "$ctl_bin" "$package_dir/files/usr/sbin/wloc-ctl"
cp "$repo_root/openwrt/files/etc/init.d/wloc-service" "$package_dir/files/etc/init.d/wloc-service"
cp "$repo_root/openwrt/files/etc/config/wloc-service" "$package_dir/files/etc/config/wloc-service"
for helper in export-mobileconfig.sh wloc-redirect-sync.sh wloc-refresh-set.sh wloc-health.sh; do
	cp "$repo_root/openwrt/files/usr/sbin/$helper" "$package_dir/files/usr/sbin/$helper"
done
chmod 0755 "$package_dir/files/usr/sbin/"* "$package_dir/files/etc/init.d/"*
mkdir -p "$package_dir/files/usr/share/wificalling-location-gateway"
printf '%s\n' standard > "$package_dir/files/usr/share/wificalling-location-gateway/runtime-variant"

cat > "$package_dir/Makefile" <<EOF
include \$(TOPDIR)/rules.mk
PKG_NAME:=wificalling-location-gateway
PKG_VERSION:=$version
PKG_RELEASE:=$release
PKG_MAINTAINER:=wificalling-location-gateway maintainers
PKG_LICENSE:=MIT
include \$(INCLUDE_DIR)/package.mk
define Package/wificalling-location-gateway
  SECTION:=net
  CATEGORY:=Network
  TITLE:=Integrated Wi-Fi Calling and WLOC Location Gateway
  EXTRA_DEPENDS:=luci-base (>=0), rpcd-mod-rpcsys (>=0), sing-box (>=0), nftables (>=0), firewall4 (>=0), kmod-nft-tproxy (>=0), kmod-nft-socket (>=0), ip-full (>=0)
  PROVIDES:=wloc-service luci-app-wificalling-location-gateway luci-app-wificalling-gateway
  CONFLICTS:=wificalling-location-gateway-lite
endef
define Package/wificalling-location-gateway/description
  Complete Wi-Fi Calling Gateway, WLOC service, control client, and unified LuCI UI.
endef
define Build/Compile
endef
define Package/wificalling-location-gateway/conffiles
/etc/config/wificalling-gateway
/etc/config/wloc-service
endef
define Package/wificalling-location-gateway/install
	\$(CP) ./files/. \$(1)/
endef
define Package/wificalling-location-gateway/preinst
#!/bin/sh
[ -n "\$\${IPKG_INSTROOT:-}" ] && exit 0
wait_for_managed_processes() {
  i=0
  while [ "\$\$i" -lt 15 ]; do
    running=
    for cmdline in /proc/[0-9]*/cmdline; do
      [ -r "\$\$cmdline" ] || continue
      first=\$\$(tr '\000' '\n' < "\$\$cmdline" 2>/dev/null | sed -n '1p')
      case "\$\$first" in
        */wloc-service) running=1; break;;
        */sing-box|*/sing-box-lite)
          tr '\000' ' ' < "\$\$cmdline" 2>/dev/null | grep -F '/var/run/wificalling-gateway/sing-box.json' >/dev/null && running=1
          [ -n "\$\$running" ] && break;;
      esac
    done
    [ -z "\$\$running" ] && return 0
    sleep 1
    i=\$\$((i + 1))
  done
  logger -t wificalling-location-gateway 'managed process did not exit before package operation'
  return 1
}
/etc/init.d/wloc-service stop >/dev/null 2>&1 || true
/etc/init.d/wificalling-gateway stop >/dev/null 2>&1 || true
wait_for_managed_processes || exit 1
rm -f /tmp/sing-box-lite /tmp/sing-box-lite.sha256 /tmp/sing-box-lite.new.* /tmp/node-health-*
exit 0
endef
define Package/wificalling-location-gateway/prerm
#!/bin/sh
[ -n "\$\${IPKG_INSTROOT:-}" ] && exit 0
wait_for_managed_processes() {
  i=0
  while [ "\$\$i" -lt 15 ]; do
    running=
    for cmdline in /proc/[0-9]*/cmdline; do
      [ -r "\$\$cmdline" ] || continue
      first=\$\$(tr '\000' '\n' < "\$\$cmdline" 2>/dev/null | sed -n '1p')
      case "\$\$first" in
        */wloc-service) running=1; break;;
        */sing-box|*/sing-box-lite)
          tr '\000' ' ' < "\$\$cmdline" 2>/dev/null | grep -F '/var/run/wificalling-gateway/sing-box.json' >/dev/null && running=1
          [ -n "\$\$running" ] && break;;
      esac
    done
    [ -z "\$\$running" ] && return 0
    sleep 1
    i=\$\$((i + 1))
  done
  logger -t wificalling-location-gateway 'managed process did not exit before package operation'
  return 1
}
/etc/init.d/wloc-service stop >/dev/null 2>&1 || true
/etc/init.d/wificalling-gateway stop >/dev/null 2>&1 || true
wait_for_managed_processes || exit 1
rm -f /tmp/sing-box-lite /tmp/sing-box-lite.sha256 /tmp/sing-box-lite.new.* /tmp/node-health-*
exit 0
endef
define Package/wificalling-location-gateway/postinst
#!/bin/sh
[ -n "\$\${IPKG_INSTROOT:-}" ] && exit 0
for required in /usr/bin/sing-box /usr/sbin/nft /usr/sbin/ip /usr/libexec/rpcd; do
  [ -e "\$\$required" ] || echo "wificalling-location-gateway: prerequisite missing: \$\$required" >&2
done
/etc/init.d/wificalling-gateway enable >/dev/null 2>&1 || true
/etc/init.d/wloc-service enable >/dev/null 2>&1 || true
mkdir -p /var/run/wificalling-gateway
chmod 0700 /var/run/wificalling-gateway
/etc/init.d/wificalling-gateway restart >/dev/null 2>&1 || true
/etc/init.d/wloc-service restart >/dev/null 2>&1 || true
rm -f /tmp/luci-indexcache.*
/etc/init.d/rpcd reload >/dev/null 2>&1 || true
exit 0
endef
\$(eval \$(call BuildPackage,wificalling-location-gateway))
EOF

case ",$variants," in
	*,lite,*)
		lite_dir="$stage/input/wificalling-location-gateway-lite"
		cp -R "$package_dir" "$lite_dir"
		printf '%s\n' lite > "$lite_dir/files/usr/share/wificalling-location-gateway/runtime-variant"
		"$repo_root/scripts/openwrt/package-singbox-lite.sh" \
			"$lite_dir/files" "$singbox_lite_bin" "$singbox_lite_sha256"
		cat > "$lite_dir/Makefile" <<EOF
include \$(TOPDIR)/rules.mk
PKG_NAME:=wificalling-location-gateway-lite
PKG_VERSION:=$version
PKG_RELEASE:=$release
PKG_MAINTAINER:=wificalling-location-gateway maintainers
PKG_LICENSE:=MIT GPL-3.0-or-later
include \$(INCLUDE_DIR)/package.mk
define Package/wificalling-location-gateway-lite
  SECTION:=net
  CATEGORY:=Network
  TITLE:=Integrated Wi-Fi Calling and WLOC Location Gateway Lite
  EXTRA_DEPENDS:=luci-base (>=0), rpcd-mod-rpcsys (>=0), ca-bundle (>=0), kmod-inet-diag (>=0), kmod-netlink-diag (>=0), kmod-tun (>=0), nftables (>=0), firewall4 (>=0), kmod-nft-tproxy (>=0), kmod-nft-socket (>=0), ip-full (>=0)
  PROVIDES:=wificalling-location-gateway sing-box wloc-service luci-app-wificalling-location-gateway luci-app-wificalling-gateway
  CONFLICTS:=wificalling-location-gateway sing-box
endef
define Package/wificalling-location-gateway-lite/description
  Complete Wi-Fi Calling Gateway and WLOC with the bundled low-memory sing-box runtime.
endef
define Build/Compile
endef
define Package/wificalling-location-gateway-lite/conffiles
/etc/config/wificalling-gateway
/etc/config/wloc-service
endef
define Package/wificalling-location-gateway-lite/install
	\$(CP) ./files/. \$(1)/
endef
define Package/wificalling-location-gateway-lite/preinst
#!/bin/sh
[ -n "\$\${IPKG_INSTROOT:-}" ] && exit 0
wait_for_managed_processes() {
  i=0
  while [ "\$\$i" -lt 15 ]; do
    running=
    for cmdline in /proc/[0-9]*/cmdline; do
      [ -r "\$\$cmdline" ] || continue
      first=\$\$(tr '\000' '\n' < "\$\$cmdline" 2>/dev/null | sed -n '1p')
      case "\$\$first" in
        */wloc-service) running=1; break;;
        */sing-box|*/sing-box-lite)
          tr '\000' ' ' < "\$\$cmdline" 2>/dev/null | grep -F '/var/run/wificalling-gateway/sing-box.json' >/dev/null && running=1
          [ -n "\$\$running" ] && break;;
      esac
    done
    [ -z "\$\$running" ] && return 0
    sleep 1
    i=\$\$((i + 1))
  done
  logger -t wificalling-location-gateway 'managed process did not exit before package operation'
  return 1
}
/etc/init.d/wloc-service stop >/dev/null 2>&1 || true
/etc/init.d/wificalling-gateway stop >/dev/null 2>&1 || true
wait_for_managed_processes || exit 1
rm -f /tmp/sing-box-lite /tmp/sing-box-lite.sha256 /tmp/sing-box-lite.new.* /tmp/node-health-*
exit 0
endef
define Package/wificalling-location-gateway-lite/prerm
#!/bin/sh
[ -n "\$\${IPKG_INSTROOT:-}" ] && exit 0
wait_for_managed_processes() {
  i=0
  while [ "\$\$i" -lt 15 ]; do
    running=
    for cmdline in /proc/[0-9]*/cmdline; do
      [ -r "\$\$cmdline" ] || continue
      first=\$\$(tr '\000' '\n' < "\$\$cmdline" 2>/dev/null | sed -n '1p')
      case "\$\$first" in
        */wloc-service) running=1; break;;
        */sing-box|*/sing-box-lite)
          tr '\000' ' ' < "\$\$cmdline" 2>/dev/null | grep -F '/var/run/wificalling-gateway/sing-box.json' >/dev/null && running=1
          [ -n "\$\$running" ] && break;;
      esac
    done
    [ -z "\$\$running" ] && return 0
    sleep 1
    i=\$\$((i + 1))
  done
  logger -t wificalling-location-gateway 'managed process did not exit before package operation'
  return 1
}
/etc/init.d/wloc-service stop >/dev/null 2>&1 || true
/etc/init.d/wificalling-gateway stop >/dev/null 2>&1 || true
wait_for_managed_processes || exit 1
rm -f /tmp/sing-box-lite /tmp/sing-box-lite.sha256 /tmp/sing-box-lite.new.* /tmp/node-health-*
exit 0
endef
define Package/wificalling-location-gateway-lite/postinst
#!/bin/sh
[ -n "\$\${IPKG_INSTROOT:-}" ] && exit 0
for required in /usr/bin/sing-box /usr/sbin/nft /usr/sbin/ip /usr/libexec/rpcd; do
  [ -e "\$\$required" ] || echo "wificalling-location-gateway-lite: prerequisite missing: \$\$required" >&2
done
/etc/init.d/wificalling-gateway enable >/dev/null 2>&1 || true
/etc/init.d/wloc-service enable >/dev/null 2>&1 || true
mkdir -p /var/run/wificalling-gateway
chmod 0700 /var/run/wificalling-gateway
/etc/init.d/wificalling-gateway restart >/dev/null 2>&1 || true
/etc/init.d/wloc-service restart >/dev/null 2>&1 || true
rm -f /tmp/luci-indexcache.*
/etc/init.d/rpcd reload >/dev/null 2>&1 || true
exit 0
endef
\$(eval \$(call BuildPackage,wificalling-location-gateway-lite))
EOF
		;;
esac

build_with_sdk() {
	label=$1
	image=$2
	variant=$3
	case "$variant" in
		standard) package=wificalling-location-gateway ;;
		lite) package=wificalling-location-gateway-lite ;;
		*) fail "unsupported build variant: $variant" ;;
	esac
	out="$stage/output/$label-$variant"
	mkdir -p "$out"
	chmod 0777 "$out"
	image_tag=${image%@*}
	docker image inspect "$image_tag" >/dev/null 2>&1 ||
		fail "pinned SDK image missing; pull explicitly: docker pull --platform linux/amd64 $image"
	docker run --rm --pull never --platform linux/amd64 --network none \
		-v "$stage/input:/input:ro" -v "$out:/output" \
		-e WLG_PACKAGE="$package" --entrypoint /bin/bash "$image" -ec '
			rm -rf "/builder/package/$WLG_PACKAGE"
			cp -a "/input/$WLG_PACKAGE" /builder/package/
			cd /builder
			printf "%s\n" "CONFIG_PACKAGE_$WLG_PACKAGE=m" >> .config
			make defconfig >/dev/null
			make "package/$WLG_PACKAGE/compile" V=s
			find bin/packages -type f \( -name "$WLG_PACKAGE*.ipk" \
				-o -name "$WLG_PACKAGE*.apk" \) -exec cp {} /output/ \;
		'
}

case ",$variants," in
	*,standard,*)
		build_with_sdk openwrt-24.10 "$OPENWRT_24_SDK" standard
		build_with_sdk openwrt-25.12 "$OPENWRT_25_SDK" standard
		;;
esac
case ",$variants," in
	*,lite,*)
		build_with_sdk openwrt-24.10 "$OPENWRT_24_SDK" lite
		build_with_sdk openwrt-25.12 "$OPENWRT_25_SDK" lite
		;;
esac

mkdir -p "$out_dir"
find "$out_dir" -maxdepth 1 -type f \( -name 'wificalling-location-gateway*.ipk' \
	-o -name 'wificalling-location-gateway*.apk' -o -name 'SHA256SUMS' \
	-o -name 'docker-matrix-report.txt' \) -delete
find "$stage/output" -type f \( -name '*.ipk' -o -name '*.apk' \) -exec cp {} "$out_dir/" \;
count=$(find "$out_dir" -maxdepth 1 -type f \( -name 'wificalling-location-gateway*.ipk' \
	-o -name 'wificalling-location-gateway*.apk' \) | wc -l | tr -d ' ')
expected_count=2
[ "$variants" = standard,lite ] && expected_count=4
[ "$count" -eq "$expected_count" ] || fail "expected $expected_count integrated packages, found $count"
(cd "$out_dir" && shasum -a 256 wificalling-location-gateway*.ipk \
	wificalling-location-gateway*.apk > SHA256SUMS)
printf 'release packages: %s\n' "$out_dir"
