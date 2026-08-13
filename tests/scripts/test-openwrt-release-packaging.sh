#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
builder="$repo_root/scripts/openwrt/build-release-packages.sh"
matrix="$repo_root/scripts/openwrt/verify-docker-matrix.sh"
runtime_builder="$repo_root/scripts/openwrt/build-x86_64-runtime.sh"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-package-test.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}

[ -x "$builder" ] || fail "missing executable $builder"
[ -x "$matrix" ] || fail "missing executable $matrix"
[ -x "$runtime_builder" ] || fail "missing executable $runtime_builder"

printf '#!/bin/sh\nexit 0\n' > "$tmp/wloc-service"
printf '#!/bin/sh\nexit 0\n' > "$tmp/wloc-ctl"
chmod 0755 "$tmp/wloc-service" "$tmp/wloc-ctl"

plan=$(
	"$builder" --plan \
		--version 0.1.0 \
		--release 3 \
		--arch x86_64 \
		--service-bin "$tmp/wloc-service" \
		--ctl-bin "$tmp/wloc-ctl"
)

printf '%s\n' "$plan" | grep -F 'wloc-service_0.1.0-r3_x86_64.ipk' >/dev/null ||
	fail '24.10 runtime IPK must be architecture-specific'
printf '%s\n' "$plan" | grep -F 'luci-app-wificalling-location-gateway_0.1.0-r3_all.ipk' >/dev/null ||
	fail '24.10 LuCI IPK must remain architecture-independent'
printf '%s\n' "$plan" | grep -F 'wloc-service-0.1.0-r3.apk' >/dev/null ||
	fail '25.12 runtime APK must be planned'
printf '%s\n' "$plan" | grep -F 'luci-app-wificalling-location-gateway-0.1.0-r3.apk' >/dev/null ||
	fail '25.12 LuCI APK must be planned'
printf '%s\n' "$plan" | grep -F 'ghcr.io/openwrt/sdk:x86_64-24.10.8@sha256:b28d5e4087dbd3f815a8bf5440a11e54e6bbd3d7400c3729d872e7940a4a77c1' >/dev/null ||
	fail '24.10 SDK image must be immutable'
printf '%s\n' "$plan" | grep -F 'ghcr.io/openwrt/sdk:x86_64-25.12.3@sha256:a0ab488698b70d6585dc35bebb77b3f6d9523fd68873fab78a1bd19cc123cd0f' >/dev/null ||
	fail '25.12 SDK image must be immutable'

if "$builder" --plan --arch all --service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl" >"$tmp/out" 2>"$tmp/err"; then
	fail 'runtime architecture all must be rejected'
fi
grep -F 'runtime architecture must not be all or noarch' "$tmp/err" >/dev/null ||
	fail 'architecture rejection must be explicit'
if "$builder" --out-dir /tmp --arch x86_64 --service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl" >"$tmp/out" 2>"$tmp/err"; then
	fail 'broad release output directory must be rejected'
fi
grep -F 'dedicated openwrt-release directory' "$tmp/err" >/dev/null ||
	fail 'output directory rejection must be explicit'

matrix_plan=$("$matrix" --plan --dist-dir "$tmp")
for expected in \
	'OpenWrt 24.10.8|opkg|ghcr.io/openwrt/rootfs:x86_64-24.10.8' \
	'OpenWrt 25.12.3|apk|ghcr.io/openwrt/rootfs:x86_64-25.12.3' \
	'iStoreOS 24.10.5|opkg|wukongdaily/openwrt-istoreos:amd64-latest'; do
	printf '%s\n' "$matrix_plan" | grep -F "$expected" >/dev/null ||
		fail "missing Docker matrix row: $expected"
done
printf '%s\n' "$matrix_plan" | grep -F 'sha256:9972a4b4747cd136abd597475d7b88c51a49fd849d0d53f069a2f4bf446061b9' >/dev/null ||
	fail '24.10 rootfs image must be immutable'
printf '%s\n' "$matrix_plan" | grep -F 'sha256:af882e0583954fc2ceac6b081a9d214fc739cfea36a29b48795a5f15563aa3b5' >/dev/null ||
	fail '25.12 rootfs image must be immutable'
printf '%s\n' "$matrix_plan" | grep -F 'sha256:83965cb67d661a28e471c491c60efffa0bffd9bec6bf13a3f0172ffd9f46b6b3' >/dev/null ||
	fail 'iStoreOS rootfs image must be immutable'

runtime_plan=$("$runtime_builder" --plan --out-dir "$tmp")
printf '%s\n' "$runtime_plan" | grep -F 'x86_64-unknown-linux-musl' >/dev/null ||
	fail 'runtime builder must use the musl Rust target'
printf '%s\n' "$runtime_plan" | grep -F 'ghcr.io/openwrt/sdk:x86_64-24.10.8@sha256:b28d5e4087dbd3f815a8bf5440a11e54e6bbd3d7400c3729d872e7940a4a77c1' >/dev/null ||
	fail 'runtime builder must use the immutable OpenWrt toolchain image'
printf '%s\n' "$runtime_plan" | grep -F 'rust:1.90.0-slim-bookworm@sha256:64232e656c058f4468e8d024e990acff04f0fd5a5c0a88a574dc37773d7325c9' >/dev/null ||
	fail 'runtime builder must use the immutable Rust image'

printf '%s\n' 'OpenWrt release packaging tests passed'
