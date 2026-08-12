# Agent handoff: Issue 17

## Identity and scope

- Source agent ID: zcode-wloc-service
- Capabilities used: rust,security
- Branch: codex/issue-17-wloc-service
- Checkpoint parent: 22c7f71f174aa0b95fb30b83c5ad7a16009f7e41
- Updated at (UTC): 2026-08-12T01:48:44Z
- Credentials included: no

## Objective

Deliver the offline WLOC service control plane, Phase 3 patch core, and the
Phase 4 MITM foundation, wired into a runnable daemon, so the system is ready
for router deployment and iPhone validation once the real sing-box probe,
nftables redirect, and hardware are available.

## Completed

- **Phase 3 WLOC patch core** (`src/wloc/`): clean-room protocol notes
  (`docs/protocol/WLOC_PROTOCOL_NOTES.md`) derived from the owner's existing
  implementation. A bounded protobuf parser, the Location sub-message
  replacement (fields 1/2/3/4/5/6/11/12; `int64(coord * 1e8)` fixed-point,
  two's-complement varint for negatives), byte-for-byte preservation of every
  other field, recursive WifiDevice (field 2) / CellResponse (fields 22/24)
  patching with missing-location append, root drop fields 3/4/33, and
  synthetic/marker envelope re-wrapping. Fail-open: any error or invalid
  coordinate leaves the response unchanged.
- **Phase 4 MITM foundation**: `src/mitm/` with an in-memory root CA
  (`CaBundle`) and per-host leaf issuance for the approved hosts (keys never
  persisted), a fail-closed `MitmCertResolver` (only the two approved SNIs get
  a leaf), and `MitmProxy` that terminates TLS, bridges HTTP/2 to the real
  upstream (verified against webpki roots), and patches `/clls/wloc` response
  bodies. Proven end to end: a client that trusts the root CA receives a
  response patched to the target coordinates over a real TLS + H2 round trip
  to a mock upstream; non-WLOC paths pass through byte-for-byte.
- **Daemon integration**: `wloc-service` generates and exports the root CA
  (PEM), listens for client traffic on `WLOC_PROXY_PORT` (default 8443), and
  shares the freshest Geo patch target with the service via a Mutex sink
  (`WlocService::with_patch_sink`). Verified live: curl trusting the exported
  CA completes TLS 1.3 + h2 with the MITM leaf for gs-loc.apple.com
  (subject CN=gs-loc.apple.com, issuer wloc-service root CA, verify ok).
- **Phase 0 fully closed**: Issues #1/#2/#3/#15 merged plus session docs.
- **Real Geo provider**: `georesolver/http.rs` verified live against
  ip-api.com; daemon defaults to it (`WLOC_GEO_PROVIDER=stub` forces stub).
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

- **Router adapters**: real sing-box exit probe (through the node outbound)
  and the nftables/procd redirect that points the test device's
  gs-loc.apple.com traffic at `WLOC_PROXY_PORT` are not yet implemented; the
  daemon uses the env-configurable stub probe and the real Geo HTTP provider.
- **Real-device validation**: requires the AX6S router and an iPhone. Sequence:
  install the exported root CA, bind the test device IP, run the daemon with
  the real probe, and follow DEVELOPMENT_TEST_PLAN.md Phase 6 (fixed device
  IP, CA fingerprint check, no Shadowrocket, UK/US/HK node switching, Safari
  cert check, UDP 500/4500 untouched).
- The CA is regenerated per daemon run; for stable trust across restarts the
  root CA should be persisted on the router storage (private key stays on the
  device, never in the repo).
- `main` branch protection still requires GitHub Pro; squash-merge +
  CODEOWNERS + CI + Agent rules remain the compensating controls.

## Next executable steps

1. Implement the real sing-box exit probe (temporary outbound config) and the
   nftables redirect + watchdog behind the existing traits; the daemon then
   uses real exit data to drive the Geo provider and the patch target.
2. Persist the root CA on the router (private key on-device only) and add a
   `wloc-service` procd integration with the proxy port.
3. Re-run the pinned AArch64 cross-build and produce an installable `.ipk`
   (OpenWrt scaffolding already in place).
4. Deploy on the AX6S and follow the Phase 6 iPhone validation sequence.
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
