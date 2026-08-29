#!/bin/sh
set -eu

dist_dir=
plan_only=0
report=
OPENWRT_24_ROOTFS='ghcr.io/openwrt/rootfs:x86_64-24.10.8@sha256:9972a4b4747cd136abd597475d7b88c51a49fd849d0d53f069a2f4bf446061b9'
OPENWRT_25_ROOTFS='ghcr.io/openwrt/rootfs:x86_64-25.12.3@sha256:af882e0583954fc2ceac6b081a9d214fc739cfea36a29b48795a5f15563aa3b5'
ISTOREOS_24_ROOTFS='wukongdaily/openwrt-istoreos:amd64-latest@sha256:83965cb67d661a28e471c491c60efffa0bffd9bec6bf13a3f0172ffd9f46b6b3'
AX6S_24_ROOTFS='ghcr.io/openwrt/rootfs:aarch64_generic-24.10.5@sha256:93f980c266b9b68e3085f3eee7909c04f1dc4061047558e18a9ef12aec43efa9'

fail() {
	printf 'verify-docker-matrix: %s\n' "$*" >&2
	exit 2
}

while [ "$#" -gt 0 ]; do
	case "$1" in
		--dist-dir) [ "$#" -ge 2 ] || fail 'missing --dist-dir value'; dist_dir=$2; shift 2 ;;
		--report) [ "$#" -ge 2 ] || fail 'missing --report value'; report=$2; shift 2 ;;
		--plan) plan_only=1; shift ;;
		-h|--help) echo 'Usage: verify-docker-matrix.sh [--plan] --dist-dir DIR [--report FILE]'; exit 0 ;;
		*) fail "unknown argument: $1" ;;
	esac
done

[ -n "$dist_dir" ] || fail '--dist-dir is required'

for variant in standard lite; do
	printf '%s|%s|%s|variant=%s\n' 'Redmi AX6S / OpenWrt 24.10.5' opkg "$AX6S_24_ROOTFS" "$variant"
	printf '%s|%s|%s|variant=%s\n' 'OpenWrt 24.10.8' opkg "$OPENWRT_24_ROOTFS" "$variant"
	printf '%s|%s|%s|variant=%s\n' 'OpenWrt 25.12.3' apk "$OPENWRT_25_ROOTFS" "$variant"
	printf '%s|%s|%s|variant=%s\n' 'iStoreOS 24.10.5' opkg "$ISTOREOS_24_ROOTFS" "$variant"
done
[ "$plan_only" -eq 0 ] || exit 0

command -v docker >/dev/null 2>&1 || fail 'docker is required'
case "$dist_dir" in /*) ;; *) fail '--dist-dir must be absolute' ;; esac
[ -d "$dist_dir" ] || fail "package directory does not exist: $dist_dir"
report=${report:-$dist_dir/docker-matrix-report.txt}
[ -f "$dist_dir/SHA256SUMS" ] || fail 'release SHA256SUMS not found'
if command -v sha256sum >/dev/null 2>&1; then
	(cd "$dist_dir" && sha256sum -c SHA256SUMS)
else
	(cd "$dist_dir" && shasum -a 256 -c SHA256SUMS)
fi

manifest_entries=$(awk 'NF == 2 { print $2 }' "$dist_dir/SHA256SUMS")
[ "$(printf '%s\n' "$manifest_entries" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 6 ] ||
	fail 'SHA256SUMS must list exactly six packages: three targets for each variant'
case "$manifest_entries" in *'/'*) fail 'SHA256SUMS package names must be basenames' ;; esac

select_manifest_package() {
	pattern=$1
	label=$2
	selected=$(printf '%s\n' "$manifest_entries" | grep -E "$pattern" || true)
	[ "$(printf '%s\n' "$selected" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1 ] ||
		fail "SHA256SUMS must list exactly one $label"
	printf '%s/%s\n' "$dist_dir" "$selected"
}

ax6s_standard_package=$(select_manifest_package '^wificalling-location-gateway_[^/]*_aarch64_cortex-a53\.ipk$' 'Standard AX6S AArch64 IPK')
ax6s_lite_package=$(select_manifest_package '^wificalling-location-gateway-lite_[^/]*_aarch64_cortex-a53\.ipk$' 'Lite AX6S AArch64 IPK')
ipk_standard_package=$(select_manifest_package '^wificalling-location-gateway_[^/]*_x86_64\.ipk$' 'Standard x86_64 IPK')
ipk_lite_package=$(select_manifest_package '^wificalling-location-gateway-lite_[^/]*_x86_64\.ipk$' 'Lite x86_64 IPK')
apk_standard_package=$(select_manifest_package '^wificalling-location-gateway-[0-9][^/]*\.apk$' 'Standard x86_64 APK')
apk_lite_package=$(select_manifest_package '^wificalling-location-gateway-lite-[0-9][^/]*\.apk$' 'Lite x86_64 APK')

find "$dist_dir" -maxdepth 1 -type f \( -name 'wificalling-location-gateway*.ipk' -o -name 'wificalling-location-gateway*.apk' \) -print |
	while IFS= read -r candidate; do
		basename=${candidate##*/}
		printf '%s\n' "$manifest_entries" | grep -Fx "$basename" >/dev/null ||
			fail "unexpected release package not listed in SHA256SUMS: $basename"
	done

tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-docker-matrix.XXXXXX")
trap 'for name in $containers; do docker rm -f "$name" >/dev/null 2>&1 || true; done; rm -rf "$tmp"' EXIT HUP INT TERM
containers=
: > "$tmp/report"

run_case() {
	name=$1
	display=$2
	manager=$3
	image=$4
	package_path=$5
	variant=$6
	platform=$7
	package_arch=$8
	case "$variant" in
		standard) package_name=wificalling-location-gateway ;;
		lite) package_name=wificalling-location-gateway-lite ;;
		*) fail "unsupported matrix variant: $variant" ;;
	esac
	container="wloc-matrix-${name}-$$"
	containers="$containers $container"
	image_tag=${image%@*}
	docker image inspect "$image_tag" >/dev/null 2>&1 || fail "missing Docker image: $image"
	if [ "$manager" = apk ]; then
		# Resolve dependencies before OpenWrt's firewall starts. Once init has
		# applied its default policy, Docker Desktop's translated egress is no
		# longer available inside this minimal rootfs.
		docker run -d --rm --privileged --pull never --platform "$platform" \
			--name "$container" -v "$dist_dir:/packages:ro" \
			-e "WLG_PACKAGE_BASENAME=${package_path##*/}" \
			--entrypoint /bin/sh "$image" -c \
			'mkdir -p /usr/sbin; [ -e /usr/sbin/ip ] || ln -s /sbin/ip /usr/sbin/ip; apk add --allow-untrusted "/packages/$WLG_PACKAGE_BASENAME" >/tmp/wlg-apk-install.log && exec /sbin/init' >/dev/null
	else
		docker run -d --rm --privileged --pull never --platform "$platform" \
			--name "$container" -v "$dist_dir:/packages:ro" \
			--entrypoint /sbin/init "$image" >/dev/null
	fi

	ready=0
	# 90 s: the apk case pre-installs the package before init, and a slow
	# network can stall that dependency resolution past 45 s without the
	# package being at fault.
	for _attempt in $(seq 1 90); do
		if docker exec "$container" /bin/sh -c 'ubus list system >/dev/null 2>&1'; then
			ready=1
			break
		fi
		sleep 1
	done
	[ "$ready" -eq 1 ] || fail "$display did not finish booting"

	if [ "$manager" = opkg ]; then
		# The minimal rootfs image intentionally ships without an opkg
		# architecture stanza. Register the architecture reported by the image;
		# production firmware already has this in /etc/opkg.conf.
		docker exec "$container" /bin/sh -c '
			for package in ca-bundle firewall4 ip-full nftables luci-base rpcd-mod-rpcsys kmod-inet-diag kmod-netlink-diag kmod-tun kmod-nft-tproxy kmod-nft-socket; do
				printf "Package: %s\nVersion: 0-docker-smoke\nArchitecture: all\nStatus: install ok installed\n\n" "$package" >> /usr/lib/opkg/status
				: > "/usr/lib/opkg/info/$package.list"
			done
		'
		if [ "$variant" = standard ]; then
			docker exec "$container" /bin/sh -c '
				printf "Package: sing-box\nVersion: 0-docker-smoke\nArchitecture: all\nStatus: install ok installed\n\n" >> /usr/lib/opkg/status
				printf "%s\n" /usr/bin/sing-box > /usr/lib/opkg/info/sing-box.list
				printf "#!/bin/sh\nwhile :; do sleep 60; done\n" > /usr/bin/sing-box
				chmod 0755 /usr/bin/sing-box
			'
		fi
		runtime_arch=$(docker exec "$container" /bin/sh -c '. /etc/openwrt_release; printf "%s" "$DISTRIB_ARCH"')
		docker exec "$container" opkg --add-arch all:1 --add-arch "$runtime_arch:50" \
			--add-arch "$package_arch:100" install --force-depends \
			"/packages/${package_path##*/}" >/dev/null
	fi
	if [ "$variant" = standard ]; then
		# Standard deliberately reuses the Gateway's single sing-box process.
		# The rootfs smoke image has no Gateway daemon, so provide only that
		# process shape before asserting that WLOC starts.
		docker exec "$container" /bin/sh -c \
			'/usr/bin/sing-box run -c /var/run/wificalling-gateway/sing-box.json >/tmp/wlg-sing-box.log 2>&1 &'
	fi
	# A production install gets this from LuCI. The minimal rootfs has no
	# Gateway device policy, so configure one manual-mode test device before
	# restarting the daemon.
	docker exec "$container" /bin/sh -c \
		"uci set wloc-service.main.assigned_device='192.0.2.10'; uci set wloc-service.main.geo_source='manual'; uci commit wloc-service"
	# The rootfs has no WAN lease, so keep DNS out of this package smoke test.
	# Production still uses the package's real resolver path.
	docker exec "$container" /bin/sh -c \
		"mkdir -p /usr/local/bin; printf '#!/bin/sh\\necho \"Address: 17.253.87.203\"\\n' > /usr/local/bin/nslookup; chmod 0755 /usr/local/bin/nslookup"
	# The minimal 25.12 rootfs image ships without /etc/config/network;
	# production firmware always defines a LAN subnet, which the redirect
	# helper needs to validate the router's IPv4 ingress.
	docker exec "$container" /bin/sh -c \
		'[ -s /etc/config/network ] && grep -q "config interface.lan" /etc/config/network || { printf "config interface lan\n\toption device br-lan\n\toption proto static\n\toption ipaddr 192.168.1.1\n\toption netmask 255.255.255.0\n" > /etc/config/network; }'

	if [ "$manager" = opkg ]; then
		# --add-arch applies only to the install invocation in these minimal
		# rootfs images, so inspect the package manager's persisted file list.
		owned=$(docker exec "$container" cat "/usr/lib/opkg/info/$package_name.list")
	else
		owned=$(docker exec "$container" apk info -L "$package_name")
	fi
	if [ "$variant" = standard ]; then
		if printf '%s\n' "$owned" | grep -E '^/?usr/bin/sing-box$' >/dev/null; then
			fail "$display standard package unexpectedly owns /usr/bin/sing-box"
		fi
	else
		printf '%s\n' "$owned" | grep -E '^/?usr/bin/sing-box$' >/dev/null ||
			fail "$display Lite package did not install /usr/bin/sing-box"
		docker exec "$container" /usr/bin/sing-box version >/dev/null ||
			fail "$display Lite sing-box runtime is not executable"
	fi

	docker exec "$container" /etc/init.d/wloc-service enable
	docker exec "$container" /etc/init.d/wloc-service restart
	socket_ready=0
	for _attempt in 1 2 3 4 5 6 7 8 9 10; do
		if docker exec "$container" test -S /var/run/wloc-service/control.sock; then
			socket_ready=1
			break
		fi
		sleep 1
	done
	[ "$socket_ready" -eq 1 ] || fail "$display did not create its control socket"
	status=$(docker exec "$container" /usr/sbin/wloc-ctl status)
	printf '%s\n' "$status" | grep -F '"api_version"' | grep -F 'wloc.service/v1' >/dev/null ||
		fail "$display returned an invalid control response"
	release=$(docker exec "$container" /bin/sh -c '. /etc/openwrt_release; printf "%s %s %s" "$DISTRIB_ID" "$DISTRIB_RELEASE" "$DISTRIB_ARCH"')
	printf '%s|%s|%s|installed|started|socket-ok|status-ok\n' "$display" "$release" "$variant" >> "$tmp/report"
	docker rm -f "$container" >/dev/null
	containers=$(printf '%s' "$containers" | sed "s/ $container//")
}

for variant in standard lite; do
	case "$variant" in
		standard)
			ax6s_package=$ax6s_standard_package
			ipk_package=$ipk_standard_package
			apk_package=$apk_standard_package
			;;
		lite)
			ax6s_package=$ax6s_lite_package
			ipk_package=$ipk_lite_package
			apk_package=$apk_lite_package
			;;
	esac
	run_case "ax6s2410-$variant" 'Redmi AX6S / OpenWrt 24.10.5' opkg "$AX6S_24_ROOTFS" \
		"$ax6s_package" "$variant" linux/aarch64_generic aarch64_cortex-a53
	run_case "openwrt2410-$variant" 'OpenWrt 24.10.8' opkg "$OPENWRT_24_ROOTFS" \
		"$ipk_package" "$variant" linux/amd64 x86_64
	run_case "openwrt2512-$variant" 'OpenWrt 25.12.3' apk "$OPENWRT_25_ROOTFS" \
		"$apk_package" "$variant" linux/amd64 x86_64
	run_case "istoreos2410-$variant" 'iStoreOS 24.10.5' opkg "$ISTOREOS_24_ROOTFS" \
		"$ipk_package" "$variant" linux/amd64 x86_64
done

cp "$tmp/report" "$report"
cat "$report"
