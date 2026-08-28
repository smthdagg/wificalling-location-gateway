# Rust route pre-development audit

Date: 2026-08-11

## Scope

This audit covers only the Rust route spike for `wificalling-location-gateway`.
It does not authorize WLOC parser implementation, response patching, CA issuance,
MITM, OpenWrt packaging, or device testing.

## Decision

Rust is accepted as the implementation language candidate for the next reviewed
migration Issue. The language, dependency, MSRV, native-size, advisory, license,
and source-scope gates pass locally. The AX6S/OpenWrt cross-build gate is not
accepted by this owner review until the repository contains a reproducible
script, retained log, and measurable target artifact. This does not release the
separate protocol, CA, MITM, packaging, or device gates.

Go remains a removable comparison scaffold for this local audit only. Its
deletion belongs in a separate Issue/PR so the language migration, CI changes,
and rollback evidence receive an independent review.

## Evidence

- Repository gate: `./scripts/ci/verify.sh` passed.
- Rust gate: `./scripts/ci/verify-rust.sh` passed.
- Rust tests: 8 tests passed across `tests/rust_spike_contract.rs` and
  `tests/rust_spike_policy.rs`.
- Native release binary:
  `target/release/wloc-gateway-spike` = 951,504 bytes under Rust 1.90.0.
- A prior note recorded an offline OpenWrt 24.10.8 `mediatek/mt7622` stripped
  target artifact of 1,118,872 bytes. Current owner review cannot verify that
  claim from the working tree: no retained AArch64 ELF artifact, build script,
  build log, or OpenWrt cross linker is currently discoverable.
- `cargo audit 0.22.2` scanned 54 lockfile dependencies against 1,207 RustSec
  advisories and reported no vulnerability.
- `cargo deny 0.20.2` passed advisories, bans, licenses, and sources checks.
- The corrected H2 test propagates handshake errors; it no longer treats a
  broken pipe as success.

## Primary documentation checked

- OpenWrt 24.10 Rust feed:
  `https://raw.githubusercontent.com/openwrt/packages/openwrt-24.10/lang/rust/Makefile`
  declares `PKG_VERSION:=1.90.0` and wires OpenWrt toolchain variables into
  Rust target configuration.
- Rust target support:
  `https://doc.rust-lang.org/rustc/platform-support/aarch64-unknown-linux-musl.html`
  defines `aarch64-unknown-linux-musl` as the Linux musl ARM64 target and
  documents cross-compilation linker requirements.
- rustls:
  `https://docs.rs/rustls/latest/rustls/` and
  `https://docs.rs/crate/rustls/latest/features` document the default
  `aws-lc-rs` provider and feature-controlled provider selection.
- h2:
  `https://docs.rs/crate/h2/latest` documents HTTP/2 scope and leaves TCP/TLS
  handling to callers.
- prost:
  `https://docs.rs/prost/latest/prost/` documents current MSRV and the
  `protoc`/`prost-build` split.

## Local code evidence

- `Cargo.toml` pins `rust-version = "1.90"` for OpenWrt 24.10 compatibility.
- `Cargo.toml` disables `rustls` default features and selects
  `features = ["ring", "std", "tls12"]`.
- `Cargo.toml` deliberately has no package license field because the repository
  has not granted an open-source license. `deny.toml` ignores only the
  unpublished private workspace crate while enforcing the dependency allowlist.
- `Cargo.toml` avoids `prost-build`, so the spike does not require runtime or
  target-side protobuf code generation.
- `src/lib.rs` keeps WLOC host scope to the six exact hostnames established by
  `DEVELOPMENT_TEST_PLAN.md` and `docs/security/WLOC_THREAT_MODEL.md`.
- `src/lib.rs` contains no `unsafe` block and its protobuf message is an opaque
  synthetic byte payload with no inferred private WLOC schema.
- `src/lib.rs` builds both rustls server and upstream client configurations with
  explicit H2 ALPN. The empty client trust store is intentionally fail-closed
  for this dependency spike; production trust-store loading is not implemented.
- `scripts/ci/verify.sh` now invokes `scripts/ci/verify-rust.sh` when
  `Cargo.toml` exists.

## Supply-chain notes

`cargo metadata --locked` reports 54 locked dependencies. License fields contain
MIT, Apache-2.0, ISC, BSD-3-Clause, Unicode-3.0, and Apache-2.0 WITH
LLVM-exception variants. No AGPL/GPL license field appeared in the lockfile
metadata inspected locally.

`cargo tree -e features --locked` shows `ring 0.17.14` uses build dependency
`cc 1.4.2`; therefore ARM64/OpenWrt validation must include native toolchain
compilation, not just Rust bytecode compilation.

## Findings

### P1 — OpenWrt/AArch64 size claim is not reproducible from the working tree

The repository currently contains no script or retained artifact that proves the
reported 1,118,872-byte target binary. Current PATH also lacks
`aarch64-linux-musl-gcc` and `aarch64-openwrt-linux-musl-gcc`; only the native
Mach-O ARM64 artifact is present. This blocks deleting the Go comparison
scaffold and blocks claiming the OpenWrt/AX6S size gate as closed.

Required fix: add a pinned, non-interactive OpenWrt SDK/toolchain cross-build
script plus a checked-in audit log or generated report containing toolchain
URL/digest, Rust version, target triple, `file`, `wc -c`, and dynamic
dependency output.

### P1 — H2/TLS smoke is still not end-to-end TLS-over-H2

The H2 test now propagates handshake errors and no longer reclassifies broken
pipe shutdown as success. The TLS test builds rustls server/client configs with
H2 ALPN. However, they remain separate tests: no self-signed leaf certificate,
SAN restriction, ALPN negotiation, TLS handshake, and then H2 stream on the TLS
transport are exercised in one path.

Required fix before proxy implementation: add an in-memory TLS + ALPN + H2
round trip using generated non-persistent test certificates whose SANs are only
the approved WLOC hostnames.

### P2 — Source unsafe scan is shallow

`verify-rust.sh` scans only direct `src/` files for `unsafe {`. That is
acceptable for this spike, but dependencies such as `ring` intentionally contain
native/unsafe code. Production review needs either `cargo geiger` or a manual
unsafe/native dependency exception register.

### P2 — Advisory DB is network-refreshed

`cargo audit` and `cargo deny` pass now, but the advisory database is refreshed
from the network. CI should pin tool versions and either cache or record the
advisory database revision for reproducible audits.

## Remaining blockers

- The AArch64 artifact has not been reproduced from a repository script and has
  not run under QEMU or on the target router.
- No OpenWrt package, procd service, seccomp profile, or memory/FD/CPU runtime
  measurement has been built.
- Rust line/branch coverage is not yet measured; the 80% product-code policy
  is therefore not claimed by this spike.
- The dependency audit database is network-refreshed; reproducible CI pins the
  audit tool versions, but an offline advisory snapshot policy is still needed.
- `scripts/dev_readiness.py --profile implementation` remains blocked by:
  - `fixtures/wloc/manifest.json`
  - `docs/protocol/WLOC_PROTOCOL_CONTRACT.md`
  - `docs/adr/0002-ipv6-strategy.md`
  - `docs/adr/0003-fail-open-slo.md`
  - local ShellCheck/Go tool availability

## Go / No-Go

Go for next stage:

1. Create a dedicated GitHub Issue for reviewed Go-to-Rust scaffold migration.
2. Add a reproducible OpenWrt package build and run the artifact on QEMU/router.
3. Add Rust coverage tooling and enforce at least 80% once product code starts.
4. Close the fixture, protocol-contract, IPv6, and fail-open SLO gates.

No-go:

- Do not implement WLOC parser/patching.
- Do not generate or commit CA material.
- Do not intercept live device traffic.
- Do not delete the Go scaffold outside a dedicated, reviewed migration Issue.

The native Rust route is small enough to justify a migration Issue. The
OpenWrt/AArch64 size gate still needs reproducible evidence before Go removal.

## Readiness score

Rust language-route readiness: **72/100**. The score passes local language
choice because MSRV, dependency locking, native size, supply-chain checks, and
failure propagation are evidenced. It does not pass migration readiness until
OpenWrt/AArch64 cross-build evidence is reproducible. It is not a
production-readiness score: runtime, packaging, coverage, protocol evidence,
and safety SLOs remain open.
