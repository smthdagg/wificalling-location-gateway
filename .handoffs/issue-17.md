# Agent handoff: Issue 17

## Identity and scope

- Source agent ID: zcode-wloc-service
- Capabilities used: rust,security
- Branch: codex/issue-17-wloc-service
- Checkpoint parent: 93547c0d5eb1faa430c16e38f2447fa955cf1e0e
- Updated at (UTC): 2026-08-11T14:10:54Z
- Credentials included: no

## Objective

Build the WLOC service control plane and runtime boundaries so the daemon can
serve the frozen `wloc.service/v1` API over a root-only Unix socket, report
real exit/Geo evidence, and be deployed on OpenWrt once the Phase 0 gates
close. No WLOC response patching, CA, or interception is implemented.

## Completed

- **Frame codec hardening**: `FramedIo` with connection poisoning, sanitized
  I/O errors (ErrorKind only), and a single total deadline a slow trickle
  cannot reset. `MAX_CONTROL_FRAME_BYTES` deduplicated to a single transport
  source re-exported by `service::api`.
- **Response encoder**: stable snake_case error wire codes, bounded
  result/error envelopes, oversized rejection, no device/location/provider
  material.
- **Dispatcher**: `ServiceDispatch` trait + `DispatchError` mapping to stable
  envelopes (invalid_config, engine_unhealthy, redirect_present,
  cleanup_unsafe, runtime_failure, unavailable).
- **UDS server**: `ControlServer` serves the full
  read-decode-dispatch-encode-write loop over a real Unix socket; verified
  end to end with a live client (status.get, enable, disable, unknown method).
- **Runtime boundaries**: `ExitProbeRuntime` (fail-closed `observe_exit`),
  `GeoProviderRuntime` (fail-open `resolve_geo`, wrong-exit filtering).
- **Status evidence**: `ExitState` (unknown/verified/stale/unavailable) and
  `GeoState` (unavailable/fresh/uncertain) in the snapshot; coordinates never
  exposed.
- **Production composition**: `WlocService` ties state machine + transactional
  control + probe + Geo into a `ServiceDispatch` with bounded evidence cache.
- **Daemon**: `wloc-service` binary serves the control API on
  `/var/run/wloc-service/control.sock` (0600) with env-configurable stub
  adapters; live smoke-tested end to end.
- **OpenWrt scaffolding**: procd init script (0700 socket dir), UCI config,
  package Makefile, deployment README.
- **Issue #2 fixture remediation verified**: differential review confirms the
  three original findings (self-declared authorization, binary scan, schema
  trust) are remediated; 20 fixture tests + full repo verification pass; the
  remediation commit was pushed to the issue-2 branch for the two original
  reviewers' re-review.

## Files changed (since the previous checkpoint)

- `src/runtime/uds.rs`, `src/service/api.rs`, `src/service/dispatch.rs`,
  `src/service/server.rs`, `src/service/status.rs`, `src/service/mod.rs`
- `src/exitprobe/{mod,runtime}.rs`, `src/georesolver/{mod,runtime}.rs`
- `src/app.rs`, `src/bin/wloc-service.rs`, `src/lib.rs`, `Cargo.toml`,
  `.gitignore`
- `tests/runtime_uds.rs`, `tests/service_response.rs`, `tests/service_dispatch.rs`,
  `tests/server_uds.rs`, `tests/exitprobe_runtime.rs`,
  `tests/georesolver_runtime.rs`, `tests/service_status.rs`, `tests/app_service.rs`
- `openwrt/{Makefile,README.md,files/etc/init.d/wloc-service,files/etc/config/wloc-service}`
- `.handoffs/issue-17.md`

## Verification

| Command | Result | Evidence |
|---|---|---|
| `cargo test --workspace --all-targets` | Passed | 83 tests, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | no warnings |
| `cargo fmt --check` | Passed | formatted |
| `./scripts/ci/verify.sh` | Passed | 92%+ line coverage, advisories/bans/licenses ok, repository gates passed |
| live daemon smoke test | Passed | status.get (verified exit, fresh geo), enable->intercepting, disable->disabled, unknown method error over a real Unix socket |
| issue-2 worktree `verify.sh` | Passed | 26 Python tests, secret scan, repository gates |

## Failed attempts

- `tokio::fs::remove_file` required the `fs` feature; replaced with `std::fs`
  in the server integration test.
- Parallel tests collided on temp socket paths (clock resolution); added a
  monotonic counter to `temp_socket_path`.
- `UnixListener::bind` panicked outside the tokio reactor; moved the bind
  inside `runtime.block_on`.
- Clippy `too_many_arguments` on `WlocService::new`; replaced with a
  `WlocServiceConfig` struct.
- `ExitEvidence::Stale` was unreachable dead code (refresh always re-probes);
  removed the variant while keeping the `ExitState::Stale` wire contract.

## Unresolved decisions and blockers

- **Phase 0 hard gate**: WLOC response patching, CA installation, TLS/H2 MITM,
  and interception remain blocked until Issue #1 (license ADR, PR awaiting
  merge approval), Issue #2 (fixture governance, remediation pushed for
  re-review), and Issue #3 (threat model, PR green) close.
- Real sing-box exit probe, online Geo provider, and nftables/procd runtime
  control adapters are not yet implemented; the daemon uses env-configurable
  stub adapters.
- OpenWrt AArch64 cross-build evidence exists (issue-15) but must be re-run
  reproducibly in this branch before packaging.
- Real-device testing requires the AX6S router and an iPhone; the phased
  sequence in DEVELOPMENT_TEST_PLAN.md Phase 6 must be followed.
- `main` branch protection still requires GitHub Pro; squash-merge +
  CODEOWNERS + CI + Agent rules remain the compensating controls.

## Next executable steps

1. Re-review Issue #2 fixture remediation (two original reviewers), then merge
   Phase 0 PRs (Issue #1, #2, #3) to unlock WLOC protocol work.
2. Implement the real OpenWrt adapters: sing-box exit probe, Geo HTTP
   provider, nftables redirect + watchdog behind the existing traits.
3. Wire `WlocService` with real adapters in the daemon; re-run the pinned
   AArch64 cross-build and produce an installable `.ipk`.
4. Deploy on the AX6S and follow the Phase 6 iPhone validation sequence.

## Capabilities required for the next Agent

- rust
- security
- openwrt

## Environment assumptions

- Rust 1.90.0 (MSRV), cargo, cargo-audit, cargo-deny, llvm-cov available.
- Unix (macOS or Linux) for the UDS integration tests.
- No network access, device, CA, or production fixture is required for the
  offline control-plane work.

## Security and privacy notes

- No API keys, tokens, private keys, `.env` values, raw captures, device
  identifiers, or precise user locations are included.
- Status snapshots never carry coordinates, device addresses, or provider
  payloads; the exit evidence model withdraws observations on probe failure.
- No WLOC response patching, CA installation, traffic interception, packaging,
  or deployment is implemented in this checkpoint.
