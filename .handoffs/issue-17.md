# Agent handoff: Issue 17

## Identity and scope

- Source agent ID: zcode-wloc-service
- Capabilities used: rust,security
- Branch: codex/issue-17-wloc-service
- Checkpoint parent: cabd076fd7b6a912ab87e88619fcecfb39261fc5
- Updated at (UTC): 2026-08-12T07:29:15Z
- Credentials included: no

## Objective

Deliver the router-side WLOC location spoofing on the AX6S. **Verified
end-to-end on the real device**: an iPhone behind the redirect receives
patched Apple network-location coordinates (agent log `response body 508 ->
553 bytes, is_wloc=true, patch=true`) and shows the target location (US
Broadlands from the stub exit) instead of the real one.

## Completed

- **Auto mode follows the real node exit**: `SingBoxProbe` reads the
  Gateway's running sing-box.json, selects the outbound bound to the test
  device (route rules, with first-non-direct fallback), spins up a temporary
  sing-box with a local HTTP proxy, and probes the node's real exit IP via an
  IP echo. Verified live: the auto mode now resolves the UK node exit
  13.40.106.250 to GB/London (geo fresh), instead of the stub.
- **Manual/auto location switching verified on the device**: `geo.set`
  accepts a place query (geocoded online via Nominatim) or explicit
  coordinates and publishes the preset to the proxy patch target; `geo.clear`
  returns to automatic node-following. Both verified live: the iPhone shows
  the manual preset (Hong Kong) and, in auto mode, the node-exit location
  (US Broadlands from the stub exit).
- **Complete multi-app location coverage verified**: with the test device
  bound to a node and TPROXY-intercepted by the Gateway, and with Passwall
  bypassing it (`WFC_GATEWAY_BYPASS`) and no device VPN running, Apple Maps,
  Amap, and Google Maps all show the node location (UK/London) consistently;
  web IP lookups show the sing-box node exit (13.40.106.250 AWS London); the
  iPhone clock follows the patched timezone. Operational requirements are
  documented in AX6S_DEPLOYMENT.md (device binding, Passwall bypass, no
  Cloudflare WARP/device VPN, Safari cache clearing).
- **Real-device WLOC rewrite works**: following the Home-Location-Endpoint
  reference, three protocol fixes made Apple accept the proxied request and
  the rewrite land:
  1. Forward a single Content-Length (duplicate Content-Length made Apple
     return 400 "Bad Request").
  2. Force `Accept-Encoding: identity` so the BlockBSSIDApple protobuf comes
     back uncompressed (gzip bodies cannot be rewritten).
  3. Recognize the real 10-byte opaque header framing
     (`[0:2]=0x0001`, `[6:10]=u32 BE block length`) and recompute the block
     length after the rewrite so locationd never reads a truncated body.
- **Phase 3 WLOC patch core** (`src/wloc/`): clean-room protocol notes
  (`docs/protocol/WLOC_PROTOCOL_NOTES.md`) derived from the owner's existing
  implementation. A bounded protobuf parser, the Location sub-message
  replacement (fields 1/2/3/4/5/6/11/12; `int64(coord * 1e8)` fixed-point,
  two's-complement varint for negatives), byte-for-byte preservation of every
  other field, recursive WifiDevice (field 2) / CellResponse (fields 22/24)
  patching with missing-location append, root drop fields 3/4/33, and
  10-byte/synthetic/marker envelope re-wrapping. Fail-open: any error or
  invalid coordinate leaves the response unchanged.
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

- **Real sing-box exit probe**: the daemon still uses the env-configurable
  stub probe, so the patched location follows the stub exit (default 8.8.8.8
  -> US). Wiring the real probe through the node outbound will make the
  location follow the selected UK/US/HK node. The redirect uses a static
  nftables set refreshed by cron (`wloc-refresh-set.sh`); a dnsmasq
  nftset/address hijack was attempted but the ImmortalWrt dnsmasq address
  override did not take effect (stop-dns-rebind left enabled was ruled out;
  left as an enhancement).
- **Full Phase 6 validation**: iPhone showed the patched US location; the
  remaining sequence (UK/US/HK node switching, Safari cert check, UDP
  500/4500 untouched, rollback) still needs to be exercised.
- The root CA is persisted on-device (`/etc/wloc-service/ca.{pem,key}`, key
  mode 0600); iPhone trust survives restarts.
- `main` branch protection still requires GitHub Pro; squash-merge +
  CODEOWNERS + CI + Agent rules remain the compensating controls.

## Next executable steps

1. Implement the real sing-box exit probe (temporary outbound config) behind
   the existing `ExitProbeRuntime` trait so the Geo provider and patch target
   follow the selected node's country.
2. Exercise the full Phase 6 sequence on the AX6S (UK/US/HK switching, Safari
   cert check, UDP 500/4500 untouched, rollback).
3. Build the installable `.ipk` from the OpenWrt scaffolding.

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
