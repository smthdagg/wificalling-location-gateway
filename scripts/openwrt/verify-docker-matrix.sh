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

printf '%s|%s|%s\n' 'Redmi AX6S / OpenWrt 24.10.5' opkg "$AX6S_24_ROOTFS"
printf '%s|%s|%s\n' 'OpenWrt 24.10.8' opkg "$OPENWRT_24_ROOTFS"
printf '%s|%s|%s\n' 'OpenWrt 25.12.3' apk "$OPENWRT_25_ROOTFS"
printf '%s|%s|%s\n' 'iStoreOS 24.10.5' opkg "$ISTOREOS_24_ROOTFS"
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
[ "$(printf '%s\n' "$manifest_entries" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 3 ] ||
	fail 'SHA256SUMS must list exactly the three release packages'
case "$manifest_entries" in *'/'*) fail 'SHA256SUMS package names must be basenames' ;; esac

select_manifest_package() {
	pattern=$1
	label=$2
	selected=$(printf '%s\n' "$manifest_entries" | grep -E "$pattern" || true)
	[ "$(printf '%s\n' "$selected" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1 ] ||
		fail "SHA256SUMS must list exactly one $label"
	printf '%s/%s\n' "$dist_dir" "$selected"
}

ax6s_package=$(select_manifest_package '^wificalling-location-gateway.*_aarch64_cortex-a53\.ipk$' 'AX6S AArch64 IPK')
ipk_package=$(select_manifest_package '^wificalling-location-gateway.*_x86_64\.ipk$' 'x86_64 IPK')
apk_package=$(select_manifest_package '^wificalling-location-gateway.*\.apk$' 'x86_64 APK')

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
	platform=$6
	package_arch=$7
	install_mode=installed
	container="wloc-matrix-${name}-$$"
	containers="$containers $container"
	docker image inspect "$image" >/dev/null 2>&1 || fail "missing Docker image: $image"
	docker run -d --rm --privileged --pull never --platform "$platform" \
		--name "$container" -v "$dist_dir:/packages:ro" \
		--entrypoint /sbin/init "$image" >/dev/null

	ready=0
	for _attempt in 1 2 3 4 5 6 7 8 9 10; do
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
			for package in firewall4 ip-full nftables sing-box luci-base rpcd-mod-rpcsys kmod-nft-tproxy kmod-nft-socket; do
				printf "Package: %s\nVersion: 0-docker-smoke\nArchitecture: all\nStatus: install ok installed\n\n" "$package" >> /usr/lib/opkg/status
				: > "/usr/lib/opkg/info/$package.list"
			done
		'
		docker exec "$container" opkg --add-arch all:1 --add-arch "$package_arch:100" install --force-depends \
			"/packages/${package_path##*/}" >/dev/null
	else
		docker exec "$container" apk add --allow-untrusted --no-network \
			--force-missing-repositories --force-broken-world \
			--repositories-file /dev/null \
			"/packages/${package_path##*/}" >/dev/null
		if ! docker exec "$container" test -f /etc/init.d/wificalling-location-gateway; then
			# The minimal rootfs has no APK repository indexes for the declared
			# OpenWrt dependencies. If apk's solver leaves the package absent,
			# extract the already-verified payload and continue the lifecycle
			# smoke test; production firmware uses its populated APK indexes.
			docker exec "$container" /bin/sh -c \
				"cd / && apk extract --allow-untrusted /packages/${package_path##*/}" >/dev/null
			install_mode=payload-extracted
		fi
	fi

	# The minimal OpenWrt rootfs images intentionally do not include a
	# sing-box provider.  Install a bounded smoke-test provider so this matrix
	# exercises the integrated package lifecycle without downloading or
	# embedding a second production sing-box binary.  AX6S production testing
	# separately verifies the real tiny/lite/PassWall provider path.
	docker exec "$container" /bin/sh -c '
		mkdir -p /usr/bin /var/run/wloc-service
		printf "%s\\n" "#!/bin/sh" \
			"case \"\\\$1\" in" \
			"  version) echo \"sing-box version 1.12.0\" ;;" \
			"  check) [ \"\\\$2\" = -c ] && [ -f \"\\\$3\" ] ;;" \
			"  run) while :; do sleep 60; done ;;" \
			"  *) exit 0 ;;" \
			"esac" > /usr/bin/sing-box
		chmod 0755 /usr/bin/sing-box
		ln -sf /usr/bin/sing-box /usr/bin/sing-box-tiny
		rm -f /usr/bin/nslookup
		printf "%s\\n" "#!/bin/sh" "echo Address: 17.0.0.1" > /usr/bin/nslookup
		chmod 0755 /usr/bin/nslookup
		printf "%s\\n" "{}" > /var/run/wloc-service/sing-box.json
		uci -q set wloc-service.smoke=device || true
		uci -q set wloc-service.smoke.label=matrix-smoke || true
		uci -q set wloc-service.smoke.assigned_device=192.168.1.100 || true
		uci -q set wloc-service.smoke.node_ref=default || true
		uci -q set wloc-service.smoke.enabled=1 || true
		uci -q commit wloc-service || true
		uci -q set network.lan.ipaddr=192.168.1.1 || true
		uci -q commit network || true
	'

	# Exercise the shipped integrated Gateway/WLOC lifecycle. The legacy
	# wloc-service init
	# facade must not be the matrix's primary startup path.
	docker exec "$container" /etc/init.d/wificalling-location-gateway enable
	# Wait for the minimal rootfs network/UCI bootstrap before the first
	# provider/DNS readiness check; production procd performs this ordering.
	sleep 3
	docker exec "$container" /etc/init.d/wificalling-location-gateway restart
	docker exec "$container" /etc/init.d/wificalling-location-gateway status >/dev/null
	socket_ready=0
	for _attempt in 1 2 3 4 5 6 7 8 9 10; do
		if docker exec "$container" test -S /var/run/wloc-service/control.sock; then
			socket_ready=1
			break
		fi
		sleep 1
	done
	if [ "$socket_ready" -ne 1 ]; then
		docker exec "$container" /bin/sh -c \
			'cat /var/run/wificalling-location-gateway/supervisor.json 2>/dev/null || true; command -v nslookup || true; nslookup gs-loc.apple.com 223.5.5.5 2>&1 || true; uci -q get network.lan.ipaddr || true' >&2
		fail "$display did not create its control socket"
	fi
	status=$(docker exec "$container" /usr/sbin/wloc-ctl status)
	printf '%s\n' "$status" | grep -F '"api_version"' | grep -F 'wloc.service/v1' >/dev/null ||
		fail "$display returned an invalid control response"
	release=$(docker exec "$container" /bin/sh -c '. /etc/openwrt_release; printf "%s %s %s" "$DISTRIB_ID" "$DISTRIB_RELEASE" "$DISTRIB_ARCH"')
	printf '%s|%s|%s|started|socket-ok|status-ok\n' "$display" "$release" "$install_mode" >> "$tmp/report"
	docker rm -f "$container" >/dev/null
	containers=$(printf '%s' "$containers" | sed "s/ $container//")
}

run_case ax6s2410 'Redmi AX6S / OpenWrt 24.10.5' opkg "$AX6S_24_ROOTFS" \
	"$ax6s_package" linux/aarch64_generic aarch64_cortex-a53
run_case openwrt2410 'OpenWrt 24.10.8' opkg "$OPENWRT_24_ROOTFS" \
	"$ipk_package" linux/amd64 x86_64
run_case openwrt2512 'OpenWrt 25.12.3' apk "$OPENWRT_25_ROOTFS" \
	"$apk_package" linux/amd64 x86_64
run_case istoreos2410 'iStoreOS 24.10.5' opkg "$ISTOREOS_24_ROOTFS" \
	"$ipk_package" linux/amd64 x86_64

cp "$tmp/report" "$report"
cat "$report"
