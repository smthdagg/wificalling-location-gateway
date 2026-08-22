#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo_root"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required for Rust verification" >&2
  exit 127
fi

if command -v rustup >/dev/null 2>&1 \
  && rustup toolchain list | grep -q '^1\.90\.0-'; then
  export RUSTUP_TOOLCHAIN=1.90.0
fi

rust_version=$(rustc --version)
case "$rust_version" in
  "rustc 1.90.0 "*) ;;
  *)
    echo "Rust 1.90.0 is required; found: $rust_version" >&2
    exit 1
    ;;
esac

for audit_tool in cargo-audit cargo-deny cargo-llvm-cov; do
  if ! command -v "$audit_tool" >/dev/null 2>&1; then
    echo "$audit_tool is required for Rust verification" >&2
    exit 127
  fi
done

cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo llvm-cov --workspace --all-targets --locked --fail-under-lines 80
cargo build --locked --release --bins
cargo audit --file Cargo.lock
cargo deny check

binary="target/release/wloc-gateway-spike"
if [ ! -x "$binary" ]; then
  echo "missing release binary: $binary" >&2
  exit 1
fi

size_bytes="$(wc -c < "$binary" | tr -d ' ')"
limit_bytes="$((8 * 1024 * 1024))"
if [ "$size_bytes" -gt "$limit_bytes" ]; then
  echo "release binary exceeds 8MiB: ${size_bytes} bytes" >&2
  exit 1
fi

"$binary" >/dev/null
./scripts/ci/verify-resource-budgets.sh

if grep -R "unsafe[[:space:]]*{" src >/dev/null 2>&1; then
  echo "unsafe marker found in Rust spike scope" >&2
  exit 1
fi

echo "Rust release binary size: ${size_bytes} bytes"
