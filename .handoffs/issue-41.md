# Agent handoff: Issue 41

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: integration,security,test,openwrt,ui
- Branch: codex/issue-41-v2-release-codex-v2-lead-20260821133002-404596fe
- Checkpoint parent: 501dea9
- Updated at (UTC): 2026-08-21T14:30:00Z
- Credentials included: no

## Objective

Complete V2.0 unified Gateway/WLOC integration, multi-device profiles, LuCI
management/diagnostics/update surfaces, low-resource AX6S packaging, migration,
rollback, documentation and release acceptance without claiming hardware gates
that have not been observed.

## Completed

- Unified supervisor owns Gateway/WLOC lifecycle and LuCI restart paths.
- Profile CRUD is UCI-backed with validation, commit/revert behavior and
  per-profile status/log diagnostics.
- Redirect destination leases expire and are refreshed after process failure;
  WLOC remains scoped to the assigned device, exact Apple hosts and TCP 443.
- Raw WLOC/debug output is opt-in, bounded and privacy-safe; support bundles
  redact profile/device/location material.
- Added shared sing-box provider resolution for AX6S-tested tiny/lite or
  PassWall binaries without packaging a duplicate full-size binary.
- Updated authoritative development, deployment, package, release, migration
  and rollback documentation to require removing old application packages
  before AX6S installation while retaining the selected provider.
- Added package and migration contract tests and integrated them into CI.
- Created PR #60 for review; it explicitly keeps real AX6S gates open.

## Files changed

- `src/`, `openwrt/`, `scripts/`, `tests/`
- `DEVELOPMENT_TEST_PLAN.md`
- `docs/adr/0002-v2-unified-runtime-and-device-profiles.md`
- `docs/deployment/AX6S_DEPLOYMENT.md`
- `docs/operations/V2_SINGBOX_RUNTIME.md`
- `docs/operations/V2_COMPONENT_UPDATE.md`
- `docs/releases/RELEASE_PROCESS.md`
- `docs/releases/V2.0_TASK_BREAKDOWN.md`
- `docs/testing/`

## Verification

| Command | Result | Evidence |
|---|---|---|
| `./scripts/ci/verify.sh` | Passed | 83 Python tests, all Rust targets, package/UI/shell/security/resource/update checks |
| Rust coverage | Passed | 80.11% line coverage, above the 80% gate |
| `tests/scripts/test-singbox-runtime.sh` | Passed | provider selection, version validation and explicit invalid-provider rejection |
| `tests/scripts/test-ax6s-migration-contract.sh` | Passed | backup, stop/disable, remove-only-old-apps, post-removal space check and install ordering |
| `tests/scripts/test-standalone-ax6s-package.sh` | Passed | architecture-specific package layout and provider warning |
| `git diff --check` | Passed | no whitespace errors |

## Failed attempts

- The first PR contract check failed because the required Issue #41 handoff
  capsule was absent; this capsule and the official handoff state publication
  are the recovery action.
- Local commit hooks report that `lefthook` is unavailable; repository gates
  remain the authoritative local verification.

## Unresolved decisions and blockers

- Real AX6S RSS/CPU/storage/startup measurements are still pending.
- AX6S migration, reboot, interrupted-update recovery and rollback are still
  pending; old application packages must be removed first because storage is
  insufficient for coexistence.
- Architecture-correct AArch64 package build, final checksums and Docker
  release-artifact matrix evidence are still required before tagging/publishing.
- No final V2 tag/feed/release should be published before Issue #41 acceptance
  is green.

## Next executable steps

1. Obtain the redacted AX6S staging evidence using the deployment checklist.
2. Back up UCI/CA, stop and disable old services, remove only old application
   packages, preserve the selected tiny/lite/PassWall provider, and recheck
   `/overlay` and `/tmp` space.
3. Install the architecture-correct package, verify provider path/version,
   exercise multi-profile auto/manual/disabled/degraded flows, and measure
   resource/log/storage budgets.
4. Rehearse upgrade, interruption recovery and rollback; record only redacted
   aggregate evidence in Issue #41/PR #60.
5. Obtain independent security review and project-lead go/no-go before merge,
   tag, feed signing or release publication.

## Capabilities required for the next Agent

- openwrt,test,security,integration,ui

## Environment assumptions

- The branch is based on `origin/main` commit `501dea9` and must be reviewed
  through PR #60 before integration.
- The AX6S test device is on a compatible 24.x OpenWrt/ImmortalWrt image and
  has a tested sing-box tiny/lite or PassWall provider available.
- Host-side tests do not emulate procd, flash exhaustion, power loss, or live
  iPhone/Gateway traffic.

## Security and privacy notes

- No API keys, tokens, private keys, `.env` values, raw captures, device
  identifiers, precise locations or production traffic are included.
- The provider resolver validates absolute executable paths and only checks
  `version`; it never attaches to or mutates a PassWall-owned process/config.
- WLOC failures withdraw redirect/return to passthrough and must not create a
  default fake coordinate.
