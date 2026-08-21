# Agent handoff: Issue 35

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: rust,openwrt,test
- Branch: codex/issue-35-v2-profile-runtime-codex-v2-lead-20260821044654
- Checkpoint parent: dda770e
- Updated at (UTC): 2026-08-21T07:00:00Z
- Credentials included: no

## Objective

Implement the bounded V2-03 per-profile runtime foundations while preserving
the frozen V1 facade and the OpenWrt fail-open traffic-isolation contract.

## Completed

- Added bounded `ProfileRuntimeManager` with independent profile phases,
  redirect cleanup, degraded passthrough, duplicate-device rejection, and a
  shared-engine health check on every new profile admission.
- Added OpenWrt `wloc-profile-redirect.sh` with validated profile IDs/private
  IPv4 bindings, exact TCP/443 `@apple_hosts` matching, isolated nft tables,
  and profile-only cleanup.
- Updated DNS/IP set refresh to populate live profile tables even when the
  legacy `wloc_service` table is absent.
- Added redacted profile health projection and unified LuCI device-profile and
  service-status views, plus unified supervisor restart RPC/ACL wiring.
- Added bounded WLOC/Gateway logs, bounded synthesized-client cache, and
  bounded debug samples for AX6S storage/memory safety.
- Added package-builder coverage for the profile helpers.

## Files changed

- `src/service/profile_runtime.rs`, `src/service/mod.rs`
- `src/bin/wloc-service.rs`, `src/app.rs`, `src/mitm/proxy.rs`,
  `src/config/profile.rs`
- OpenWrt profile redirect, status, refresh, health, and package wiring
- LuCI device-profile/status pages, menu, RPC, and ACL wiring
- Profile/runtime/log/cache/package/UI tests
- `docs/releases/V2.0_TASK_BREAKDOWN.md`,
  `docs/api/WLOC_SERVICE_API_V2_DRAFT.md`
- `.handoffs/issue-35.md`

## Verification

- `./scripts/ci/verify.sh` — passed; advisory scan reports no advisories.
- `cargo test --all-targets` — passed.
- `cargo clippy --all-targets --all-features -- -D warnings` — passed.
- `tests/scripts/test-profile-redirect.sh` — passed.
- `tests/scripts/test-profile-status.sh` — passed.
- `tests/scripts/test-gateway-log-bound.sh` — passed.
- `tests/scripts/test-unified-supervisor.sh` — passed.
- `python3 -m unittest tests.test_v2_ui_contract tests.test_wloc_luci_mode` — passed.
- Independent review after fixes: no P0-P3 findings; handoff approved.

## Failed attempts

- The first handoff capsule used a project-specific heading and was rejected
  by the repository handoff validator; it was corrected to the canonical
  template before publication.

## Unresolved decisions and blockers

- Production still uses one `WlocService` patch/Geo context and intentionally
  refuses multi-profile runtime selection.
- No live AX6S RSS/storage measurement has been performed in this workspace.

## Explicit release boundary

The production daemon still constructs one `WlocService` patch/Geo context and
still refuses to select one profile when multiple profiles are configured.
Therefore this handoff does not claim live multi-profile node probing or
source-device-specific WLOC patch routing. The next implementation must add
the shared-engine profile dispatcher and per-profile Geo/patch sink before
enabling multiple profiles in production.

## Next executable steps

1. Integrate profile dispatcher into the unified service/API without changing
   the frozen v1 facade.
2. Add source-device-to-profile patch selection and per-profile auto/manual
   Geo refresh tests.
3. Re-run AX6S real-device memory/storage measurements before merging/release.

## Capabilities required for the next Agent

- rust
- openwrt
- network
- test
- security

## Environment assumptions

- Rust, Python 3, POSIX shell, Git, and GitHub CLI are available.
- Verification is offline/synthetic; no router, device, credential, or raw
  production traffic is required.

## Security and privacy notes

- No API keys, tokens, private keys, `.env` values, raw captures, device
  identifiers, or precise user locations are included.
- Profile health output remains redacted and profile interception remains
  restricted to exact Apple hostnames and TCP 443.
