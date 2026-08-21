# Agent handoff: Issue 36

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: rust,openwrt,network,test
- Branch: codex/issue-36-v2-profile-dispatcher-codex-v2-lead-20260821071402-acda6a62
- Checkpoint parent: 7a50ef8
- Updated at (UTC): 2026-08-21T08:32:35Z
- Credentials included: no

## Objective

Implement production multi-profile WLOC routing for Issue #36: one validated
IPv4 LAN device per profile, independent node/probe/Geo handler and patch sink,
shared Gateway/WLOC engine, fail-open behavior, and verified profile-scoped
redirect ownership.

## Completed

- Added bounded `ProfilePatchRouter` source-device dispatch with no default
  profile fallback and explicit MAC/IPv6 rejection in the current adapter.
- Added one independent WLOC probe/Geo handler per profile while retaining one
  shared `ProfileRuntimeManager`/Gateway engine.
- Added profile enable/disable/reload routing and auto/manual target isolation.
- Made every profile route start disabled and enable only after redirect
  installation and status verification.
- Removed legacy all-device redirect ownership from multi-profile mode while
  preserving the shared fwmark/local TPROXY route required by profile tables.
- Added profile `stop-all`, orphan/disabled table cleanup, stale legacy table
  cleanup, and bounded proxy-ready/activation/profile-ready startup markers.
- Added Rust and shell regression coverage, documentation, and integrated the
  profile tests into `scripts/ci/verify.sh`.

## Files changed

- `src/bin/wloc-service.rs`
- `src/service/dispatch.rs`
- `src/service/profile_dispatch.rs`
- `src/mitm/proxy.rs`
- `openwrt/files/usr/sbin/wloc-profile-redirect.sh`
- `openwrt/files/usr/sbin/wloc-redirect-sync.sh`
- `openwrt/files/usr/sbin/wloc-refresh-set.sh`
- `openwrt/files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh`
- `openwrt/files/etc/init.d/wloc-service`
- `openwrt/files/etc/init.d/wificalling-location-gateway`
- `openwrt/luci-app-wificalling-location-gateway/files/usr/libexec/rpcd/luci.wloc`
- `tests/profile_dispatch.rs`
- `tests/scripts/test-profile-redirect.sh`
- `tests/scripts/test-unified-supervisor.sh`
- `scripts/ci/verify.sh`
- `docs/testing/V2_PROFILE_DISPATCHER.tdd.md`
- `docs/api/WLOC_SERVICE_API_V2_DRAFT.md`
- `docs/releases/V2.0_TASK_BREAKDOWN.md`

## Verification

| Command | Result | Evidence |
|---|---|---|
| `./scripts/ci/verify.sh` | Passed | Full repository gates; 71 Python tests, OpenWrt/AX6S packaging, Rust audit and release checks |
| `cargo test --all-targets` | Passed | All Rust unit/integration targets passed; one documented live-network test ignored |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed | No warnings |
| `cargo llvm-cov --workspace --all-targets --locked --fail-under-lines 80` | Passed | 80.17% total Rust line coverage on final full run |
| `./tests/scripts/test-profile-redirect.sh` | Passed | Route install/remove, stale global cleanup, disabled/orphan cleanup, scope guards |
| `./tests/scripts/test-unified-supervisor.sh` | Passed | Profile route/readiness contract, legacy-table exclusion, cleanup ordering |
| Independent reviewer | APPROVE | No P0/P1/P2; one non-blocking P3 test marker hygiene suggestion |

## Failed attempts

- Initial full verification was below the 80% Rust coverage gate; added
  profile-group lifecycle tests and restored coverage above the threshold.
- Independent review found and fixed three successive P1 classes: legacy
  global redirect overlap, missing shared policy route/stale table cleanup,
  and profile redirect installation before proxy/supervisor readiness.

## Unresolved decisions and blockers

- Live v2 profile CRUD/mutation dispatch and the unified LuCI device management
  UI remain Issue #37 by design; the current V1 control facade targets the
  deterministic default profile.
- Structured log UI/support bundle remains Issue #38.
- Component update/compatibility/rollback remains Issue #39.
- AX6S real-device resource evidence remains Issue #40.
- Migration rehearsal and final release acceptance remain Issue #41.
- Non-blocking hygiene: the Rust profile-group lifecycle test uses the default
  readiness marker path and may log a permission warning in an unprivileged
  checkout; production startup marker handling is bounded and fail-safe.

## Next executable steps

1. Run `scripts/agent-handoff.sh 36 codex-v2-lead rust,openwrt,network,test`.
2. Open a PR against `main` with `Closes #36` and this handoff capsule.
3. Require the independent review and all GitHub checks before merging.
4. After merge, take Issue #37 and implement live profile mutation plus the
   unified LuCI settings/device/status UI.

## Capabilities required for the next Agent

- `rust`, `openwrt`, `network`, `test`, and security-sensitive nft/procd review.

## Environment assumptions

- OpenWrt provides UCI, nft, ip, procd, a root-owned `/var/run` hierarchy,
  and the packaged profile/global helper paths.
- Multi-profile runtime bindings are IPv4 addresses until a future adapter
  explicitly supports MAC/IPv6 bindings.
- `WLOC_SUPERVISED=1` is set by the unified procd supervisor; standalone
  daemon mode activates only after the proxy listener is bound.

## Security and privacy notes

- No API keys, tokens, private keys, `.env` values, raw captures, device
  identifiers, or precise user locations are included.
- Redirect scope remains profile-assigned device + exact Apple WLOC hostnames
  + TCP 443; UDP 500/4500 and the stable Gateway nft namespace are untouched.
- Unknown sources, disabled/degraded profiles, invalid Geo targets, missing
  readiness markers, and runtime failures fail open without fake coordinates.
