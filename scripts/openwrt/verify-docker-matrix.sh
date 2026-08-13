#!/bin/sh
set -eu

dist_dir=
plan_only=0
report=
OPENWRT_24_ROOTFS='ghcr.io/openwrt/rootfs:x86_64-24.10.8@sha256:9972a4b4747cd136abd597475d7b88c51a49fd849d0d53f069a2f4bf446061b9'
OPENWRT_25_ROOTFS='ghcr.io/openwrt/rootfs:x86_64-25.12.3@sha256:af882e0583954fc2ceac6b081a9d214fc739cfea36a29b48795a5f15563aa3b5'
ISTOREOS_24_ROOTFS='wukongdaily/openwrt-istoreos:amd64-latest@sha256:83965cb67d661a28e471c491c60efffa0bffd9bec6bf13a3f0172ffd9f46b6b3'

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

printf '%s|%s|%s\n' 'OpenWrt 24.10.8' opkg "$OPENWRT_24_ROOTFS"
printf '%s|%s|%s\n' 'OpenWrt 25.12.3' apk "$OPENWRT_25_ROOTFS"
printf '%s|%s|%s\n' 'iStoreOS 24.10.5' opkg "$ISTOREOS_24_ROOTFS"
[ "$plan_only" -eq 0 ] || exit 0

command -v docker >/dev/null 2>&1 || fail 'docker is required'
case "$dist_dir" in /*) ;; *) fail '--dist-dir must be absolute' ;; esac
[ -d "$dist_dir" ] || fail "package directory does not exist: $dist_dir"
report=${report:-$dist_dir/docker-matrix-report.txt}

ipk_package=$(find "$dist_dir" -maxdepth 1 -type f -name 'wificalling-location-gateway*_x86_64.ipk' -print -quit)
apk_package=$(find "$dist_dir" -maxdepth 1 -type f -name 'wificalling-location-gateway*.apk' -print -quit)
[ -n "$ipk_package" ] || fail 'integrated x86_64 IPK not found'
[ -n "$apk_package" ] || fail 'integrated x86_64 APK not found'

tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-docker-matrix.XXXXXX")
trap 'for name in $containers; do docker rm -f "$name" >/dev/null 2>&1 || true; done; rm -rf "$tmp"' EXIT HUP INT TERM
containers=
: > "$tmp/report"
mkdir -p "$tmp/empty" "$tmp/stub-output"
chmod 0777 "$tmp/stub-output"

# The rootfs smoke test is deliberately offline. Generate one test-only APK
# provider for dependencies that are absent from the minimal image; this APK
# is never copied into the release directory.
apk_sdk='ghcr.io/openwrt/sdk:x86_64-25.12.3@sha256:a0ab488698b70d6585dc35bebb77b3f6d9523fd68873fab78a1bd19cc123cd0f'
docker image inspect "$apk_sdk" >/dev/null 2>&1 || fail "missing Docker image: $apk_sdk"
docker run --rm --pull never --platform linux/amd64 --network none \
	-v "$tmp:/state" --entrypoint /bin/sh "$apk_sdk" -ec '
		/builder/staging_dir/host/bin/apk mkpkg \
			--info "name:wloc-docker-smoke-deps" \
			--info "version:1-r1" --info "arch:noarch" --info "license:MIT" \
			--info "provides:firewall4 ip-full nftables sing-box luci-base rpcd-mod-rpcsys kmod-nft-tproxy kmod-nft-socket" \
			--files /state/empty --output /state/stub-output/wloc-docker-smoke-deps.apk
	'

run_case() {
	name=$1
	display=$2
	manager=$3
	image=$4
	container="wloc-matrix-${name}-$$"
	containers="$containers $container"
	docker image inspect "$image" >/dev/null 2>&1 || fail "missing Docker image: $image"
	docker run -d --rm --privileged --pull never --platform linux/amd64 \
		--name "$container" -v "$dist_dir:/packages:ro" \
		-v "$tmp/stub-output:/smoke:ro" \
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
		docker exec "$container" opkg --add-arch all:1 --add-arch x86_64:100 install --force-depends \
			"/packages/${ipk_package##*/}" >/dev/null
	else
		docker exec "$container" apk add --allow-untrusted --no-network \
			--force-missing-repositories --repositories-file /dev/null \
			/smoke/wloc-docker-smoke-deps.apk \
			"/packages/${apk_package##*/}" >/dev/null
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
	printf '%s|%s|installed|started|socket-ok|status-ok\n' "$display" "$release" >> "$tmp/report"
	docker rm -f "$container" >/dev/null
	containers=$(printf '%s' "$containers" | sed "s/ $container//")
}

run_case openwrt2410 'OpenWrt 24.10.8' opkg "$OPENWRT_24_ROOTFS"
run_case openwrt2512 'OpenWrt 25.12.3' apk "$OPENWRT_25_ROOTFS"
run_case istoreos2410 'iStoreOS 24.10.5' opkg "$ISTOREOS_24_ROOTFS"

cp "$tmp/report" "$report"
cat "$report"
