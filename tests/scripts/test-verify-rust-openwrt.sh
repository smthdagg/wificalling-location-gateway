#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
verify_script="$repo_root/scripts/ci/verify-rust-openwrt.sh"
tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/verify-rust-openwrt-test.XXXXXX")
trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

assert_contains() {
    file=$1
    pattern=$2
    grep -F -- "$pattern" "$file" >/dev/null || fail "expected '$pattern' in $file"
}

[ -x "$verify_script" ] || fail "missing executable $verify_script"

mkdir -p "$tmpdir/bin"

cat >"$tmpdir/bin/curl" <<'EOF'
#!/bin/sh
set -eu
echo "curl $*" >>"$OPENWRT_CROSS_TEST_LOG"
output=
while [ "$#" -gt 0 ]; do
    if [ "$1" = "-o" ]; then
        output=$2
        shift 2
    else
        shift
    fi
done
[ -n "$output" ]
printf 'synthetic checked archive\n' >"$output"
EOF
chmod +x "$tmpdir/bin/curl"

cat >"$tmpdir/bin/shasum" <<'EOF'
#!/bin/sh
set -eu
echo "shasum $*" >>"$OPENWRT_CROSS_TEST_LOG"
[ "${OPENWRT_CROSS_TEST_BAD_SHA:-0}" != 1 ]
cat >/dev/null
EOF
chmod +x "$tmpdir/bin/shasum"

cat >"$tmpdir/bin/docker" <<'EOF'
#!/bin/sh
set -eu
printf 'docker %s\n' "$*" | tr '\n\t' '  ' | tr -s ' ' >>"$OPENWRT_CROSS_TEST_LOG"
if [ "$1" = image ] && [ "$2" = inspect ]; then
    printf '%s\n' 'rust@sha256:64232e656c058f4468e8d024e990acff04f0fd5a5c0a88a574dc37773d7325c9'
    case " $* " in
        *" rust@sha256:"*) exit "${OPENWRT_CROSS_TEST_DIGEST_STATUS:-${OPENWRT_CROSS_TEST_IMAGE_STATUS:-0}}" ;;
        *) exit "${OPENWRT_CROSS_TEST_TAG_STATUS:-${OPENWRT_CROSS_TEST_IMAGE_STATUS:-0}}" ;;
    esac
fi
if [ "$1" = run ]; then
    case " $* " in
        *" rustup target add "*)
            mkdir -p "$OPENWRT_CROSS_TEST_STATE/toolchain/bin"
            mkdir -p "$OPENWRT_CROSS_TEST_STATE/rust-target"
            mkdir -p "$OPENWRT_CROSS_TEST_STATE/cargo"
            : >"$OPENWRT_CROSS_TEST_STATE/prepared"
            ;;
        *" cargo build "*)
            mkdir -p "$OPENWRT_CROSS_TEST_STATE/output"
            artifact="$OPENWRT_CROSS_TEST_STATE/output/wloc-gateway-spike"
            if [ "${OPENWRT_CROSS_TEST_OVERSIZE:-0}" = 1 ]; then
                dd if=/dev/zero of="$artifact" bs=1 count=0 seek=8388609 2>/dev/null
            else
                printf 'elf' >"$artifact"
            fi
            cat >"$OPENWRT_CROSS_TEST_STATE/output/report.txt" <<'REPORT'
ELF_HEADER:   Class:                             ELF64
ELF_HEADER:   Machine:                           AArch64
NEEDED: Shared library: [libgcc_s.so.1]
NEEDED: Shared library: [libc.so]
REPORT
            ;;
    esac
    exit 0
fi
exit 2
EOF
chmod +x "$tmpdir/bin/docker"

run_case() {
    name=$1
    shift
    state="$tmpdir/wloc-rust-openwrt-$name"
    mkdir -p "$state"
    : >"$tmpdir/$name.calls"
    env PATH="$tmpdir/bin:/usr/bin:/bin" \
        OPENWRT_CROSS_CACHE_DIR="$state" \
        OPENWRT_CROSS_TEST_STATE="$state" \
        OPENWRT_CROSS_TEST_LOG="$tmpdir/$name.calls" \
        "$@" /bin/sh "$verify_script" >"$tmpdir/$name.out" 2>&1
}

if run_case dangerous-cache env OPENWRT_CROSS_CACHE_DIR=/; then
    fail "a broad cache directory must be rejected"
fi
assert_contains "$tmpdir/dangerous-cache.out" "refusing non-dedicated OPENWRT_CROSS_CACHE_DIR: /"
if [ -s "$tmpdir/dangerous-cache.calls" ]; then
    fail "dangerous cache validation must happen before curl or Docker"
fi

if ! run_case happy env; then
    cat "$tmpdir/happy.out" >&2
    cat "$tmpdir/happy.calls" >&2
    exit 1
fi
assert_contains "$tmpdir/happy.calls" "curl -fL --retry 3"
assert_contains "$tmpdir/happy.calls" "openwrt-toolchain-24.10.8-mediatek-mt7622_gcc-13.3.0_musl.Linux-x86_64.tar.zst"
assert_contains "$tmpdir/happy.calls" "shasum -a 256 -c -"
assert_contains "$tmpdir/happy.calls" "docker image inspect rust:1.90.0-slim-bookworm"
assert_contains "$tmpdir/happy.calls" "rustup target add --toolchain 1.90.0 aarch64-unknown-linux-musl"
assert_contains "$tmpdir/happy.calls" "cargo fetch --locked --target aarch64-unknown-linux-musl"
assert_contains "$tmpdir/happy.calls" "--network none"
assert_contains "$tmpdir/happy.calls" "target-cpu=cortex-a53"
assert_contains "$tmpdir/happy.calls" "cargo build --offline --locked --release"
assert_contains "$tmpdir/happy.calls" "--target aarch64-unknown-linux-musl --bin wloc-gateway-spike"
assert_contains "$tmpdir/happy.calls" "aarch64-openwrt-linux-musl-strip"
assert_contains "$tmpdir/happy.calls" "aarch64-openwrt-linux-musl-readelf"
assert_contains "$tmpdir/happy.out" "OpenWrt Rust binary size: 3 bytes"
assert_contains "$tmpdir/happy.out" "Class:                             ELF64"
assert_contains "$tmpdir/happy.out" "Machine:                           AArch64"
assert_contains "$tmpdir/happy.out" "Shared library: [libc.so]"

if run_case bad-sha env OPENWRT_CROSS_TEST_BAD_SHA=1; then
    fail "checksum failure must stop verification"
fi
assert_contains "$tmpdir/bad-sha.out" "toolchain checksum verification failed"
if grep -F 'docker run' "$tmpdir/bad-sha.calls" >/dev/null; then
    fail "Docker must not run after checksum failure"
fi

if run_case missing-image env OPENWRT_CROSS_TEST_IMAGE_STATUS=1; then
    fail "missing pinned image must stop without an implicit pull"
fi
assert_contains "$tmpdir/missing-image.out" "docker pull --platform linux/amd64 rust@sha256:64232e656c058f4468e8d024e990acff04f0fd5a5c0a88a574dc37773d7325c9"
if grep -F 'docker run' "$tmpdir/missing-image.calls" >/dev/null; then
    fail "Docker must not run when the pinned image is missing"
fi

run_case digest-only env OPENWRT_CROSS_TEST_TAG_STATUS=1
assert_contains "$tmpdir/digest-only.calls" "docker run --rm --pull never --platform linux/amd64 -v"
assert_contains "$tmpdir/digest-only.calls" "rust@sha256:64232e656c058f4468e8d024e990acff04f0fd5a5c0a88a574dc37773d7325c9"

if run_case oversize env OPENWRT_CROSS_TEST_OVERSIZE=1; then
    fail "an artifact larger than 8 MiB must fail"
fi
assert_contains "$tmpdir/oversize.out" "OpenWrt binary exceeds 8MiB: 8388609 bytes"

echo 'verify-rust-openwrt tests passed'
