# Agent handoff: Issue 33

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: rust,openwrt,test
- Branch: codex/issue-33-unified-supervisor-codex-v2-lead-20260821044654
- Checkpoint parent: 601097f
- Updated at (UTC): 2026-08-21T06:16:40Z
- Credentials included: no

## Objective

Deliver the V2-02 unified Gateway/WLOC lifecycle slice as an isolated OpenWrt
component: one enabled entry point, ordered passthrough/health/redirect
transitions, fail-open WLOC fault handling, bounded recovery, stable Gateway
namespace protection, runtime control delegation, packaging, tests, and
rollback documentation.

## Completed

- Added the Rust `UnifiedSupervisor` state machine with bounded child count,
  restart budget, health interval, redirect-last ordering, and explicit
  `CleanupUnsafe` state.
- Integrated `WlocService` with the supervisor and replaced the production
  no-op redirect runtime with `OpenWrtRuntime` delegating only the WLOC-owned
  helper and `nft list table inet wloc_service` presence check.
- Added the unified procd entry point and POSIX/busybox supervisor. WLOC
  startup suppresses the legacy redirect side effect; unified health checks
  precede redirect installation; WLOC faults leave Gateway passthrough alive.
- Disabled independent child respawn in supervised mode so the outer
  supervisor owns bounded recovery.
- Updated standalone and formal release package builders/postinst scripts to
  install and enable the unified entry point while retaining legacy rollback
  scripts.
- Added shell, Rust, package, TDD evidence, and operations documentation.

## Files changed

- `src/service/supervisor.rs`, `src/app.rs`, `src/bin/wloc-service.rs`
- `openwrt/files/etc/init.d/wificalling-location-gateway`
- `openwrt/files/etc/init.d/wificalling-gateway`
- `openwrt/files/etc/init.d/wloc-service`
- `openwrt/files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh`
- `openwrt/files/usr/sbin/wloc-redirect-sync.sh`
- `scripts/openwrt/build-release-packages.sh`, `scripts/build-luci-ipk.sh`
- `tests/service_supervisor.rs`, `tests/scripts/test-unified-supervisor.sh`,
  package tests
- `docs/adr/0002-v2-unified-runtime-and-device-profiles.md`,
  `docs/operations/V2_UNIFIED_SUPERVISOR.md`,
  `docs/testing/V2_UNIFIED_SUPERVISOR.tdd.md`

## Verification

| Command | Result | Evidence |
|---|---|---|
| `cargo test --all-targets` | PASS | All Rust unit/integration targets passed |
| `cargo clippy --all-targets -- -D warnings` | PASS | No warnings |
| `./tests/scripts/test-unified-supervisor.sh` | PASS | Ordering, ownership, fail-open, namespace guards |
| `./tests/scripts/test-standalone-ax6s-package.sh` | PASS | Standalone AX6S package contents/postinst |
| `./tests/scripts/test-openwrt-release-packaging.sh` | PASS | Formal release builder uses unified entry point |
| `./scripts/ci/verify.sh` | PASS | 69 Python tests, Rust coverage 81.27% lines, OpenWrt/resource/package/security gates |

## Failed attempts

- Initial full verification found one rustfmt difference; `cargo fmt --all`
  fixed it and the complete verification was rerun successfully.
- Independent review initially found early legacy redirect installation and
  WLOC-fault Gateway shutdown; both were corrected and covered by regression
  assertions.

## Unresolved decisions and blockers

- No live AX6S install/RSS/procd run has been performed in this workspace;
  hardware acceptance remains required before a production release.
- The legacy init scripts remain as a one-release migration facade. In
  supervised mode they do not own redirect installation or child respawn, but
  the next runtime issue should replace this facade with direct child adapters
  if the product requires literal single-process ownership.
- Multi-device profile routing, unified LuCI management, bounded logs, and
  component update UI are subsequent V2 issues, not claimed by this slice.

## Next executable steps

1. Run the final independent review on this checkpoint.
2. Push this branch and open the Issue #33 PR only after review has no blocking
   P0/P1 findings.
3. Install the built package on AX6S staging and record RSS, startup, crash,
   reload, rollback, and stable Gateway table evidence.
4. Continue with V2-03 device-profile runtime routing and V2-04 LuCI status,
   logs, updates, and migration UI.

## Capabilities required for the next Agent

- `openwrt`, `rust`, `test`, and `security` for the live acceptance pass.

## Environment assumptions

- OpenWrt procd, nftables TPROXY support, `sing-box`, and the stable Gateway
  files are present on the target gateway.
- The component owns only `inet wloc_service`, its policy route, WLOC DNS
  marker, root-only WLOC socket/state, and its own children.

## Security and privacy notes

- No API keys, tokens, private keys, `.env` values, raw captures, device
  identifiers, or precise user locations are included.
- WLOC cleanup never directly names the stable Gateway nftables table and does
  not handle UDP 500/4500; WLOC failure is withdrawn before WLOC shutdown.
