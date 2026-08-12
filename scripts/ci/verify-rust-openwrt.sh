#!/bin/sh
set -eu

OPENWRT_VERSION=24.10.8
OPENWRT_TARGET=mediatek/mt7622
OPENWRT_TOOLCHAIN_ARCHIVE=openwrt-toolchain-24.10.8-mediatek-mt7622_gcc-13.3.0_musl.Linux-x86_64.tar.zst
OPENWRT_TOOLCHAIN_SHA256=fc045488375d0ff6fe6bbd0d40db44b5faced186b3e8919a400d92867171a9ad
RUST_IMAGE=rust:1.90.0-slim-bookworm@sha256:64232e656c058f4468e8d024e990acff04f0fd5a5c0a88a574dc37773d7325c9
RUST_TOOLCHAIN=1.90.0
RUST_TARGET=aarch64-unknown-linux-musl
SIZE_LIMIT_BYTES=$((8 * 1024 * 1024))
BIN_NAME=${OPENWRT_BIN_NAME:-wloc-gateway-spike}

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
if [ "${OPENWRT_CROSS_CACHE_DIR+x}" = x ] && [ -z "$OPENWRT_CROSS_CACHE_DIR" ]; then
    echo "verify-rust-openwrt: OPENWRT_CROSS_CACHE_DIR must not be empty" >&2
    exit 1
fi
cache_dir=${OPENWRT_CROSS_CACHE_DIR:-${TMPDIR:-/tmp}/wloc-rust-openwrt-cross}
download_dir="$cache_dir/downloads"
archive="$download_dir/$OPENWRT_TOOLCHAIN_ARCHIVE"
toolchain_url="https://downloads.openwrt.org/releases/$OPENWRT_VERSION/targets/$OPENWRT_TARGET/$OPENWRT_TOOLCHAIN_ARCHIVE"
output_dir="$cache_dir/output"
artifact="$output_dir/$BIN_NAME"
report="$output_dir/report.txt"

fail() {
    echo "verify-rust-openwrt: $*" >&2
    exit 1
}

# Only a visibly task-specific, absolute cache is allowed because later steps
# replace generated subdirectories. Reject broad targets before invoking any
# external command or creating any path.
case "$cache_dir" in
    /*) ;;
    *) fail "OPENWRT_CROSS_CACHE_DIR must be an absolute dedicated path" ;;
esac
case "$cache_dir" in
    *'/../'*|*/..|*'/./'*|*/.)
        fail "OPENWRT_CROSS_CACHE_DIR must not contain dot path components"
        ;;
esac
cache_name=${cache_dir##*/}
case "$cache_name" in
    wloc-rust-openwrt-?*) ;;
    *) fail "refusing non-dedicated OPENWRT_CROSS_CACHE_DIR: $cache_dir" ;;
esac
home_dir=$(CDPATH='' cd -- 2>/dev/null && pwd)
case "$cache_dir" in
    /|"$home_dir"|"$repo_root"|"$repo_root"/*)
        fail "refusing dangerous OPENWRT_CROSS_CACHE_DIR: $cache_dir"
        ;;
esac
if [ -L "$cache_dir" ]; then
    fail "OPENWRT_CROSS_CACHE_DIR must not be a symbolic link: $cache_dir"
fi

for command_name in docker curl shasum; do
    command -v "$command_name" >/dev/null 2>&1 \
        || fail "$command_name is required"
done

mkdir -p "$download_dir" "$output_dir"

if ! docker image inspect "$RUST_IMAGE" >/dev/null 2>&1; then
    fail "pinned build image is missing; fetch it explicitly with: docker pull --platform linux/amd64 $RUST_IMAGE"
fi

if [ ! -f "$archive" ]; then
    partial="$archive.partial"
    rm -f "$partial"
    if ! curl -fL --retry 3 --retry-delay 2 -o "$partial" "$toolchain_url"; then
        rm -f "$partial"
        fail "failed to download pinned OpenWrt toolchain"
    fi
    mv "$partial" "$archive"
fi

if ! printf '%s  %s\n' "$OPENWRT_TOOLCHAIN_SHA256" "$archive" \
    | shasum -a 256 -c - >/dev/null 2>&1; then
    rm -f "$archive"
    fail "toolchain checksum verification failed"
fi

# Bootstrap is the only network-enabled container phase. It installs the zstd
# extractor in the pinned disposable image, obtains the pinned Rust target, and
# resolves the already locked Cargo graph into the external cache. Product
# compilation happens in a separate, network-disabled container below.
rm -f "$cache_dir/prepared"
docker run --rm --pull never --platform linux/amd64 \
    -v "$repo_root:/src:ro" \
    -v "$cache_dir:/state" \
    -w /src \
    "$RUST_IMAGE" \
    sh -ec '
        apt-get update
        apt-get install -y --no-install-recommends zstd
        rm -f /state/toolchain.tar
        rm -rf /state/rust-target
        mkdir -p /state/rust-target /state/cargo
        extract_dir=$(mktemp -d)
        trap "rm -rf $extract_dir" EXIT HUP INT TERM
        tar --zstd -xf "/state/downloads/'"$OPENWRT_TOOLCHAIN_ARCHIVE"'" \
            -C "$extract_dir" --strip-components=1
        tar -cf /state/toolchain.tar -C "$extract_dir" .
        rustup target add --toolchain '"$RUST_TOOLCHAIN"' '"$RUST_TARGET"'
        cp -a "/usr/local/rustup/toolchains/'"$RUST_TOOLCHAIN"'-x86_64-unknown-linux-gnu/lib/rustlib/'"$RUST_TARGET"'/". \
            /state/rust-target/
        CARGO_HOME=/state/cargo cargo fetch --locked --target '"$RUST_TARGET"'
        : >/state/prepared
    '

[ -f "$cache_dir/prepared" ] || fail "bootstrap did not produce its completion marker"
rm -rf "$output_dir"
mkdir -p "$output_dir"

# This is the auditable build boundary: fixed image, no network, immutable
# source, locked/offline dependencies, and the verified OpenWrt cross toolchain.
docker run --rm --pull never --platform linux/amd64 --network none \
    -v "$repo_root:/src:ro" \
    -v "$cache_dir:/state" \
    -w /src \
    "$RUST_IMAGE" \
    sh -ec '
        rustlib=/usr/local/rustup/toolchains/'"$RUST_TOOLCHAIN"'-x86_64-unknown-linux-gnu/lib/rustlib/'"$RUST_TARGET"'
        cp -a /state/rust-target/. "$rustlib/"
        extracted_toolchain=$(mktemp -d)
        trap "rm -rf $extracted_toolchain" EXIT HUP INT TERM
        tar -xf /state/toolchain.tar -C "$extracted_toolchain"
        linker=$(find "$extracted_toolchain" \
            -path "*/bin/aarch64-openwrt-linux-musl-gcc" -print -quit)
        toolchain_root=${linker%/bin/aarch64-openwrt-linux-musl-gcc}
        archiver=$toolchain_root/bin/aarch64-openwrt-linux-musl-ar
        stripper=$toolchain_root/bin/aarch64-openwrt-linux-musl-strip
        inspector=$toolchain_root/bin/aarch64-openwrt-linux-musl-readelf
        test -x "$linker"
        test -x "$archiver"
        test -x "$stripper"
        test -x "$inspector"
        CARGO_HOME=/state/cargo \
        CARGO_TARGET_DIR=/state/target \
        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="$linker" \
        RUSTFLAGS="-C target-cpu=cortex-a53" \
        CC_aarch64_unknown_linux_musl="$linker" \
        AR_aarch64_unknown_linux_musl="$archiver" \
            cargo build --offline --locked --release \
                --target '"$RUST_TARGET"' --bin '"$BIN_NAME"'
        cp /state/target/'"$RUST_TARGET"'/release/'"$BIN_NAME"' \
            /state/output/'"$BIN_NAME"'
        "$stripper" /state/output/'"$BIN_NAME"'
        {
            "$inspector" -h /state/output/'"$BIN_NAME"' \
                | sed -n "/Class:/s/^/ELF_HEADER: /p; /Machine:/s/^/ELF_HEADER: /p"
            needed=$("$inspector" -d /state/output/'"$BIN_NAME"' \
                | sed -n "/(NEEDED)/s/^/NEEDED: /p")
            if [ -n "$needed" ]; then
                printf "%s\n" "$needed"
            else
                echo "NEEDED: none (statically linked)"
            fi
        } >/state/output/report.txt
    '

[ -f "$artifact" ] || fail "cross-build did not produce $artifact"
[ -f "$report" ] || fail "cross-build did not produce $report"

size_bytes=$(wc -c <"$artifact" | tr -d ' ')
case "$size_bytes" in
    ''|*[!0-9]*) fail "could not determine OpenWrt binary size" ;;
esac
if [ "$size_bytes" -gt "$SIZE_LIMIT_BYTES" ]; then
    fail "OpenWrt binary exceeds 8MiB: $size_bytes bytes"
fi

grep -F 'Class:' "$report" | grep -F 'ELF64' >/dev/null \
    || fail "artifact report does not identify a 64-bit ELF"
grep -F 'Machine:' "$report" | grep -F 'AArch64' >/dev/null \
    || fail "artifact report does not identify AArch64"
grep -F 'NEEDED:' "$report" >/dev/null \
    || fail "artifact report is missing dynamic dependency status"

cat "$report"
echo "OpenWrt Rust binary size: $size_bytes bytes"
echo "OpenWrt Rust cross-build output: $output_dir"
