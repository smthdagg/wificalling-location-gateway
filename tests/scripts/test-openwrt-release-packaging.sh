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

grep -F '\$\$required' "$builder" >/dev/null ||
	fail 'package post-install must preserve the full prerequisite path through Make'
grep -F 'mkdir -p /var/run/wificalling-gateway' "$builder" >/dev/null ||
	fail 'release post-install must create the volatile Gateway runtime directory before restart'
grep -F 'chmod 0700 /var/run/wificalling-gateway' "$builder" >/dev/null ||
	fail 'release post-install must restrict the Gateway runtime directory'
grep -F 'rm -f /tmp/luci-indexcache.*' "$builder" >/dev/null ||
	fail 'release post-install must invalidate every LuCI menu cache variant'
if grep -F 'wloc-docker-smoke-deps' "$matrix" >/dev/null; then
	fail 'Docker verification must use real 25.x rootfs prerequisites, not conflicting fake providers'
fi
grep -F 'SHA256SUMS' "$matrix" >/dev/null ||
	fail 'Docker verification must bind tested packages to the release checksum manifest'
grep -F 'manifest_entries=' "$matrix" >/dev/null ||
	fail 'Docker verification must select install artifacts from the checksum manifest'
grep -F 'unexpected release package not listed in SHA256SUMS' "$matrix" >/dev/null ||
	fail 'Docker verification must reject unlisted matching release packages'
if grep -F 'shasum -a 256 ./wificalling-location-gateway' "$builder" >/dev/null; then
	fail 'release builder must write basename-only SHA256SUMS entries'
fi
grep -F 'luci-app-wificalling-gateway.json' "$builder" >/dev/null ||
	fail 'integrated release builder must remove the standalone Gateway LuCI menu'

printf '#!/bin/sh\nexit 0\n' > "$tmp/wloc-service"
printf '#!/bin/sh\nexit 0\n' > "$tmp/wloc-ctl"
chmod 0755 "$tmp/wloc-service" "$tmp/wloc-ctl"

plan=$(
	"$builder" --plan \
		--arch x86_64 \
		--service-bin "$tmp/wloc-service" \
		--ctl-bin "$tmp/wloc-ctl"
)

printf '%s\n' "$plan" | grep -F 'wificalling-location-gateway_1.0.8-r1_x86_64.ipk' >/dev/null ||
	fail '24.10 must produce one architecture-specific integrated IPK'
printf '%s\n' "$plan" | grep -F 'wificalling-location-gateway-1.0.8-r1.apk (arch: x86_64)' >/dev/null ||
	fail '25.12 must produce one architecture-specific integrated APK'
if printf '%s\n' "$plan" | grep -E 'wloc-service[_-]|luci-app-wificalling-location-gateway[_-]' >/dev/null; then
	fail 'formal 1.0.8 plan must not expose split component packages'
fi
printf '%s\n' "$plan" | grep -F 'ghcr.io/openwrt/sdk:x86_64-24.10.8@sha256:b28d5e4087dbd3f815a8bf5440a11e54e6bbd3d7400c3729d872e7940a4a77c1' >/dev/null ||
	fail '24.10 SDK image must be immutable'
printf '%s\n' "$plan" | grep -F 'ghcr.io/openwrt/sdk:x86_64-25.12.3@sha256:a0ab488698b70d6585dc35bebb77b3f6d9523fd68873fab78a1bd19cc123cd0f' >/dev/null ||
	fail '25.12 SDK image must be immutable'

if "$builder" --plan --arch all --service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl" >"$tmp/out" 2>"$tmp/err"; then
	fail 'runtime architecture all must be rejected'
fi
grep -F 'runtime architecture must not be all or noarch' "$tmp/err" >/dev/null ||
	fail 'architecture rejection must be explicit'
if "$builder" --plan --arch aarch64_cortex-a53 --service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl" >"$tmp/out" 2>"$tmp/err"; then
	fail 'x86_64 SDK must reject an AArch64 package label'
fi
grep -F 'this SDK matrix currently supports x86_64 only' "$tmp/err" >/dev/null ||
	fail 'unsupported SDK architecture rejection must be explicit'
if "$builder" --out-dir /tmp --arch x86_64 --service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl" >"$tmp/out" 2>"$tmp/err"; then
	fail 'broad release output directory must be rejected'
fi
grep -F 'dedicated openwrt-release directory' "$tmp/err" >/dev/null ||
	fail 'output directory rejection must be explicit'

matrix_plan=$("$matrix" --plan --dist-dir "$tmp")
for expected in \
	'Redmi AX6S / OpenWrt 24.10.5|opkg|ghcr.io/openwrt/rootfs:aarch64_generic-24.10.5' \
	'OpenWrt 24.10.8|opkg|ghcr.io/openwrt/rootfs:x86_64-24.10.8' \
	'OpenWrt 25.12.3|apk|ghcr.io/openwrt/rootfs:x86_64-25.12.3' \
	'iStoreOS 24.10.5|opkg|wukongdaily/openwrt-istoreos:amd64-latest'; do
	printf '%s\n' "$matrix_plan" | grep -F "$expected" >/dev/null ||
		fail "missing Docker matrix row: $expected"
done
printf '%s\n' "$matrix_plan" | grep -F 'sha256:93f980c266b9b68e3085f3eee7909c04f1dc4061047558e18a9ef12aec43efa9' >/dev/null ||
	fail 'AX6S-compatible AArch64 rootfs image must be immutable'
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
