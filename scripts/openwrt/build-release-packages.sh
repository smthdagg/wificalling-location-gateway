#!/bin/sh
set -eu

OPENWRT_24_SDK='ghcr.io/openwrt/sdk:x86_64-24.10.8@sha256:b28d5e4087dbd3f815a8bf5440a11e54e6bbd3d7400c3729d872e7940a4a77c1'
OPENWRT_25_SDK='ghcr.io/openwrt/sdk:x86_64-25.12.3@sha256:a0ab488698b70d6585dc35bebb77b3f6d9523fd68873fab78a1bd19cc123cd0f'

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
version=0.1.0
release=3
arch=x86_64
service_bin=
ctl_bin=
out_dir="$repo_root/dist/openwrt-release"
plan_only=0

fail() {
	printf 'build-release-packages: %s\n' "$*" >&2
	exit 2
}

usage() {
	cat <<'EOF'
Usage: build-release-packages.sh [--plan] [options]

Options:
  --version VERSION       Package version (default: 0.1.0)
  --release RELEASE       Package release number (default: 3)
  --arch ARCH             OpenWrt runtime architecture (default: x86_64)
  --service-bin PATH      Static wloc-service binary (required)
  --ctl-bin PATH          Static wloc-ctl binary (required)
  --out-dir PATH          Output directory
  --plan                  Print the immutable build plan without Docker
EOF
}

while [ "$#" -gt 0 ]; do
	case "$1" in
		--version) [ "$#" -ge 2 ] || fail 'missing --version value'; version=$2; shift 2 ;;
		--release) [ "$#" -ge 2 ] || fail 'missing --release value'; release=$2; shift 2 ;;
		--arch) [ "$#" -ge 2 ] || fail 'missing --arch value'; arch=$2; shift 2 ;;
		--service-bin) [ "$#" -ge 2 ] || fail 'missing --service-bin value'; service_bin=$2; shift 2 ;;
		--ctl-bin) [ "$#" -ge 2 ] || fail 'missing --ctl-bin value'; ctl_bin=$2; shift 2 ;;
		--out-dir) [ "$#" -ge 2 ] || fail 'missing --out-dir value'; out_dir=$2; shift 2 ;;
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
[ -n "$service_bin" ] || fail '--service-bin is required'
[ -n "$ctl_bin" ] || fail '--ctl-bin is required'
[ -x "$service_bin" ] || fail "service binary is not executable: $service_bin"
[ -x "$ctl_bin" ] || fail "control binary is not executable: $ctl_bin"

cat <<EOF
24.10 SDK: $OPENWRT_24_SDK
25.12 SDK: $OPENWRT_25_SDK
24.10 runtime: wloc-service_${version}-r${release}_${arch}.ipk
24.10 LuCI: luci-app-wificalling-location-gateway_${version}-r${release}_all.ipk
25.12 runtime: wloc-service-${version}-r${release}.apk (arch: ${arch})
25.12 LuCI: luci-app-wificalling-location-gateway-${version}-r${release}.apk (arch: noarch)
EOF

[ "$plan_only" -eq 0 ] || exit 0
command -v docker >/dev/null 2>&1 || fail 'docker is required'
case "$out_dir" in /*) ;; *) fail '--out-dir must be absolute' ;; esac
case "${out_dir##*/}" in
	openwrt-release|wloc-openwrt-release-?*) ;;
	*) fail '--out-dir must be a dedicated openwrt-release directory' ;;
esac
case "$out_dir" in /|"$repo_root") fail 'unsafe --out-dir' ;; esac
[ ! -L "$out_dir" ] || fail '--out-dir must not be a symbolic link'

stage=$(mktemp -d "${TMPDIR:-/tmp}/wloc-openwrt-package.XXXXXX")
trap 'rm -rf "$stage"' EXIT HUP INT TERM
mkdir -p "$stage/input/wloc-service/files/usr/sbin" \
	"$stage/input/wloc-service/files/etc/init.d" \
	"$stage/input/wloc-service/files/etc/config" \
	"$stage/input/luci-app-wificalling-location-gateway/files" \
	"$stage/output"
chmod 0777 "$stage/output"

cp "$service_bin" "$stage/input/wloc-service/files/usr/sbin/wloc-service"
cp "$ctl_bin" "$stage/input/wloc-service/files/usr/sbin/wloc-ctl"
cp "$repo_root/openwrt/files/etc/init.d/wloc-service" \
	"$stage/input/wloc-service/files/etc/init.d/wloc-service"
cp "$repo_root/openwrt/files/etc/config/wloc-service" \
	"$stage/input/wloc-service/files/etc/config/wloc-service"
for helper in export-mobileconfig.sh wloc-redirect-sync.sh wloc-refresh-set.sh; do
	cp "$repo_root/openwrt/files/usr/sbin/$helper" \
		"$stage/input/wloc-service/files/usr/sbin/$helper"
done
chmod 0755 "$stage/input/wloc-service/files/usr/sbin/"* \
	"$stage/input/wloc-service/files/etc/init.d/wloc-service"
cp -R "$repo_root/openwrt/luci-app-wificalling-location-gateway/files/." \
	"$stage/input/luci-app-wificalling-location-gateway/files/"
# The runtime package owns the executable helper. The LuCI package invokes it
# through rpcd but must not install a second copy of the same path.
rm -f "$stage/input/luci-app-wificalling-location-gateway/files/usr/sbin/export-mobileconfig.sh"

cat > "$stage/input/wloc-service/Makefile" <<EOF
include \$(TOPDIR)/rules.mk
PKG_NAME:=wloc-service
PKG_VERSION:=$version
PKG_RELEASE:=$release
PKG_MAINTAINER:=wificalling-location-gateway maintainers
PKG_LICENSE:=MIT
include \$(INCLUDE_DIR)/package.mk
define Package/wloc-service
  SECTION:=net
  CATEGORY:=Network
  TITLE:=WLOC location service and control client
  DEPENDS:=+firewall4 +ip-full +nftables +sing-box
endef
define Package/wloc-service/description
  Architecture-specific WLOC daemon and local Unix-socket control client.
endef
define Build/Compile
endef
define Package/wloc-service/conffiles
/etc/config/wloc-service
endef
define Package/wloc-service/install
	\$(INSTALL_DIR) \$(1)/usr/sbin \$(1)/etc/init.d \$(1)/etc/config
	\$(INSTALL_BIN) ./files/usr/sbin/wloc-service \$(1)/usr/sbin/
	\$(INSTALL_BIN) ./files/usr/sbin/wloc-ctl \$(1)/usr/sbin/
	\$(INSTALL_BIN) ./files/usr/sbin/export-mobileconfig.sh \$(1)/usr/sbin/
	\$(INSTALL_BIN) ./files/usr/sbin/wloc-redirect-sync.sh \$(1)/usr/sbin/
	\$(INSTALL_BIN) ./files/usr/sbin/wloc-refresh-set.sh \$(1)/usr/sbin/
	\$(INSTALL_BIN) ./files/etc/init.d/wloc-service \$(1)/etc/init.d/
	\$(INSTALL_CONF) ./files/etc/config/wloc-service \$(1)/etc/config/
endef
define Package/wloc-service/postinst
#!/bin/sh
[ -n "\$\${IPKG_INSTROOT:-}" ] && exit 0
/etc/init.d/wloc-service enable >/dev/null 2>&1 || true
exit 0
endef
\$(eval \$(call BuildPackage,wloc-service))
EOF

cat > "$stage/input/luci-app-wificalling-location-gateway/Makefile" <<EOF
include \$(TOPDIR)/rules.mk
PKG_NAME:=luci-app-wificalling-location-gateway
PKG_VERSION:=$version
PKG_RELEASE:=$release
PKG_MAINTAINER:=wificalling-location-gateway maintainers
PKG_LICENSE:=MIT
include \$(INCLUDE_DIR)/package.mk
define Package/luci-app-wificalling-location-gateway
  SECTION:=luci
  CATEGORY:=LuCI
  SUBMENU:=3. Applications
  TITLE:=Wi-Fi Calling and WLOC Location Gateway UI
  PKGARCH:=all
  DEPENDS:=+wloc-service +luci-app-wificalling-gateway +luci-base +rpcd-mod-rpcsys
endef
define Build/Compile
endef
define Package/luci-app-wificalling-location-gateway/install
	\$(CP) ./files/. \$(1)/
endef
\$(eval \$(call BuildPackage,luci-app-wificalling-location-gateway))
EOF

build_with_sdk() {
	label=$1
	image=$2
	out="$stage/output/$label"
	mkdir -p "$out"
	chmod 0777 "$out"
	docker image inspect "$image" >/dev/null 2>&1 ||
		fail "pinned SDK image missing; pull explicitly: docker pull --platform linux/amd64 $image"
	docker run --rm --pull never --platform linux/amd64 --network none \
		-v "$stage/input:/input:ro" -v "$out:/output" \
		--entrypoint /bin/bash "$image" -ec '
			cp -a /input/wloc-service /builder/package/
			cp -a /input/luci-app-wificalling-location-gateway /builder/package/
			printf "%s\n" "CONFIG_PACKAGE_wloc-service=m" \
				"CONFIG_PACKAGE_luci-app-wificalling-location-gateway=m" >> /builder/.config
			cd /builder
			make defconfig >/dev/null
			make package/wloc-service/compile package/luci-app-wificalling-location-gateway/compile V=s
			find bin/packages -type f \( -name "wloc-service*.ipk" -o -name "wloc-service*.apk" \
				-o -name "luci-app-wificalling-location-gateway*.ipk" \
				-o -name "luci-app-wificalling-location-gateway*.apk" \) \
				-exec cp {} /output/ \;
		'
}

build_with_sdk openwrt-24.10 "$OPENWRT_24_SDK"
build_with_sdk openwrt-25.12 "$OPENWRT_25_SDK"

mkdir -p "$out_dir"
rm -f "$out_dir"/wloc-service*.ipk "$out_dir"/wloc-service*.apk \
	"$out_dir"/luci-app-wificalling-location-gateway*.ipk \
	"$out_dir"/luci-app-wificalling-location-gateway*.apk \
	"$out_dir"/SHA256SUMS "$out_dir"/docker-matrix-report.txt
find "$stage/output" -type f \( -name '*.ipk' -o -name '*.apk' \) -exec cp {} "$out_dir/" \;
count=$(find "$out_dir" -maxdepth 1 -type f \( -name 'wloc-service*.ipk' -o -name 'wloc-service*.apk' \
	-o -name 'luci-app-wificalling-location-gateway*.ipk' \
	-o -name 'luci-app-wificalling-location-gateway*.apk' \) | wc -l | tr -d ' ')
[ "$count" -eq 4 ] || fail "expected four packages, found $count"
(cd "$out_dir" && shasum -a 256 ./*.ipk ./*.apk > SHA256SUMS)
printf 'release packages: %s\n' "$out_dir"
