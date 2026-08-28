#!/bin/sh
set -eu

OPENWRT_SDK='ghcr.io/openwrt/sdk:x86_64-24.10.8@sha256:b28d5e4087dbd3f815a8bf5440a11e54e6bbd3d7400c3729d872e7940a4a77c1'
RUST_IMAGE='rust:1.90.0-slim-bookworm@sha256:64232e656c058f4468e8d024e990acff04f0fd5a5c0a88a574dc37773d7325c9'
RUST_TOOLCHAIN=1.90.0
RUST_TARGET=x86_64-unknown-linux-musl

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
out_dir="$repo_root/dist/runtime/x86_64"
cache_dir=${WLOC_X86_BUILD_CACHE:-${TMPDIR:-/tmp}/wloc-rust-openwrt-x86_64}
plan_only=0

fail() {
	printf 'build-x86_64-runtime: %s\n' "$*" >&2
	exit 2
}

while [ "$#" -gt 0 ]; do
	case "$1" in
		--out-dir) [ "$#" -ge 2 ] || fail 'missing --out-dir value'; out_dir=$2; shift 2 ;;
		--plan) plan_only=1; shift ;;
		-h|--help) echo 'Usage: build-x86_64-runtime.sh [--plan] [--out-dir DIR]'; exit 0 ;;
		*) fail "unknown argument: $1" ;;
	esac
done

cat <<EOF
OpenWrt SDK: $OPENWRT_SDK
Rust image: $RUST_IMAGE
Rust target: $RUST_TARGET
Output: $out_dir/wloc-service, $out_dir/wloc-ctl
EOF
[ "$plan_only" -eq 0 ] || exit 0

for path in "$out_dir" "$cache_dir"; do
	case "$path" in /*) ;; *) fail 'output and cache paths must be absolute' ;; esac
done
case "$cache_dir" in
	/|"$repo_root"|"$repo_root"/*) fail "unsafe cache directory: $cache_dir" ;;
	*/wloc-rust-openwrt-x86_64|*/wloc-rust-openwrt-x86_64-*) ;;
	*) fail "cache directory must have a wloc-rust-openwrt-x86_64 name: $cache_dir" ;;
esac
command -v docker >/dev/null 2>&1 || fail 'docker is required'
for image in "$OPENWRT_SDK" "$RUST_IMAGE"; do
	image_tag=${image%@*}
	docker image inspect "$image_tag" >/dev/null 2>&1 ||
		fail "pinned image missing; pull explicitly: docker pull --platform linux/amd64 $image"
done

mkdir -p "$cache_dir" "$out_dir"

# Export only the verified OpenWrt SDK toolchain into the task-specific cache.
docker run --rm --pull never --platform linux/amd64 --network none \
	-v "$cache_dir:/state" --entrypoint /bin/bash "$OPENWRT_SDK" -ec '
		toolchain=$(find /builder/staging_dir -maxdepth 1 -type d -name "toolchain-x86_64_gcc-*_musl" -print -quit)
		test -n "$toolchain"
		toolchain_name=${toolchain##*/}
		cd /builder
		# GCC is an SDK wrapper which resolves its loader through
		# ../../host/lib, so the toolchain and host directories are one
		# relocatable unit and must be exported together.
		tar -cf /state/openwrt-toolchain.tar \
			"staging_dir/$toolchain_name" staging_dir/host
	'

# Network is allowed only while installing the fixed Rust target and resolving
# the locked dependency graph into the external cache.
docker run --rm --pull never --platform linux/amd64 \
	-v "$repo_root:/src:ro" -v "$cache_dir:/state" -w /src "$RUST_IMAGE" \
	sh -ec '
		rm -rf /state/rust-target
		mkdir -p /state/rust-target /state/cargo
		rustup target add --toolchain '"$RUST_TOOLCHAIN"' '"$RUST_TARGET"'
		cp -a /usr/local/rustup/toolchains/'"$RUST_TOOLCHAIN"'-x86_64-unknown-linux-gnu/lib/rustlib/'"$RUST_TARGET"'/. /state/rust-target/
		CARGO_HOME=/state/cargo cargo fetch --locked --target '"$RUST_TARGET"'
	'

rm -rf "$cache_dir/target" "$cache_dir/output"
mkdir -p "$cache_dir/output"

# Compilation is offline, source is read-only, and both images are immutable.
docker run --rm --pull never --platform linux/amd64 --network none \
	-v "$repo_root:/src:ro" -v "$cache_dir:/state" -w /src "$RUST_IMAGE" \
	sh -ec '
		rustlib=/usr/local/rustup/toolchains/'"$RUST_TOOLCHAIN"'-x86_64-unknown-linux-gnu/lib/rustlib/'"$RUST_TARGET"'
		cp -a /state/rust-target/. "$rustlib/"
		toolchain=$(mktemp -d)
		trap "rm -rf $toolchain" EXIT HUP INT TERM
		tar -xf /state/openwrt-toolchain.tar -C "$toolchain"
		linker=$(find "$toolchain/staging_dir" -path "*/bin/x86_64-openwrt-linux-musl-gcc" -print -quit)
		archiver=$(find "$toolchain/staging_dir" -path "*/bin/x86_64-openwrt-linux-musl-ar" -print -quit)
		stripper=$(find "$toolchain/staging_dir" -path "*/bin/x86_64-openwrt-linux-musl-strip" -print -quit)
		inspector=$(find "$toolchain/staging_dir" -path "*/bin/x86_64-openwrt-linux-musl-readelf" -print -quit)
		test -x "$linker" && test -x "$archiver" && test -x "$stripper" && test -x "$inspector"
		CARGO_HOME=/state/cargo CARGO_TARGET_DIR=/state/target \
		CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$linker" \
		CC_x86_64_unknown_linux_musl="$linker" AR_x86_64_unknown_linux_musl="$archiver" \
		RUSTFLAGS="-C target-cpu=x86-64" \
			cargo build --offline --locked --release --target '"$RUST_TARGET"' \
				--bin wloc-service --bin wloc-ctl
		for name in wloc-service wloc-ctl; do
			cp /state/target/'"$RUST_TARGET"'/release/$name /state/output/$name
			"$stripper" /state/output/$name
			"$inspector" -h /state/output/$name | grep -F "Advanced Micro Devices X86-64" >/dev/null
			if "$inspector" -d /state/output/$name | grep -F "(NEEDED)" >/dev/null; then
				echo "$name is dynamically linked" >&2
				exit 1
			fi
		done
	'

cp "$cache_dir/output/wloc-service" "$out_dir/wloc-service"
cp "$cache_dir/output/wloc-ctl" "$out_dir/wloc-ctl"
chmod 0755 "$out_dir/wloc-service" "$out_dir/wloc-ctl"
(cd "$out_dir" && shasum -a 256 wloc-service wloc-ctl > SHA256SUMS)
printf 'x86_64 OpenWrt runtime: %s\n' "$out_dir"
