# Agent handoff: Issue 39

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: openwrt,test,security,docs
- Branch: codex/issue-39-v2-update-rollback-codex-v2-lead-20260821095228-1bc1507b
- Final local checkpoint before handoff: 659930a
- Credentials included: no

## Objective

Add a low-storage-safe component update center for the unified Gateway/WLOC
package: validate identity/compatibility/architecture/version/source/space,
preserve configuration, install without remove-first behavior, gate activation
on supervisor health, and automatically roll back a failed or interrupted
transaction.

## Completed

- Added root-only `wloc-component-update.sh` with `preflight`, `apply`,
  `recover`, and `status` actions.
- Validates regular local IPK, archive paths, package identity, V2 product
  metadata, Gateway 1.7, WLOC v2 API, target architecture, semantic package
  version, signed SHA-256 manifest, free space, downgrade authorization, and
  known-good rollback package.
- Serializes updates with a persistent lock and transaction directory; stores
  previous package and both legacy component UCI files with mode-restricted
  state.
- Preserves configuration after opkg install; restarts only the unified
  supervisor and runs health validation. Install/restart/health failures restore
  the known-good package and configuration. A simulated power-loss marker is
  recoverable by `recover`.
- Explicitly contains no `opkg remove`, `nft`, UDP 500/4500 handling, or stable
  Gateway table manipulation.
- Added compatibility metadata to both package source trees and builder control
  fields; helper is installed by canonical Makefile, standalone AX6S builder,
  and release package builder.
- Added RPC/ACL methods `update_status`, `update_preflight`, `update_apply`,
  and `update_recover`, restricted package paths to `/tmp/wloc-update/*`, and
  added LuCI Health-page controls/status for preflight, apply, and recovery.
- Added update operations documentation and TDD evidence.

## Files changed

- `openwrt/files/usr/sbin/wloc-component-update.sh`
- `openwrt/files/usr/libexec/rpcd/luci.wloc`
- `openwrt/luci-app-wificalling-location-gateway/files/usr/libexec/rpcd/luci.wloc`
- both package ACL JSON files
- both LuCI health views and i18n sources
- `openwrt/Makefile`
- `scripts/build-luci-ipk.sh`
- `scripts/openwrt/build-release-packages.sh`
- `scripts/ci/verify.sh`
- compatibility metadata files under both package source trees
- `tests/scripts/test-component-update.sh`
- `tests/scripts/test-existing-ax6s-package.sh`
- `scripts/create-update-manifest.sh`
- `tests/test_v2_diagnostics_contract.py`
- `docs/operations/V2_COMPONENT_UPDATE.md`
- `docs/testing/V2_COMPONENT_UPDATE.tdd.md`

## Verification

| Command | Result | Evidence |
|---|---|---|
| `./scripts/ci/verify.sh` | Passed | 80 Python tests, all Rust targets, OpenWrt/AX6S/release packaging, update transaction, secret scan, cargo audit |
| `cargo llvm-cov --workspace --all-targets --locked --fail-under-lines 80` | Passed | 80.16% total Rust line coverage |
| `sh tests/scripts/test-component-update.sh` | Passed | architecture rejection, unauthorized/authorized downgrade, config preservation, health rollback, interrupted recovery, low-space preflight |
| `sh tests/scripts/test-existing-ax6s-package.sh` | Passed | actual legacy AX6S package build and helper/supervisor extraction |
| `python3 -m unittest tests.test_v2_diagnostics_contract` | Passed | 6 contract tests covering helper/package/RPC/ACL/UI |
| `sh tests/scripts/test-standalone-ax6s-package.sh` | Passed | standalone package path |
| `sh tests/scripts/test-openwrt-release-packaging.sh` | Passed | release package path |
| `node --check` on both health views | Passed | mirrored LuCI syntax |
| `git diff --check` | Passed | no whitespace errors |

## Failed attempts

- The initial RED test correctly failed because the update helper was absent;
  this is recorded in TDD evidence.
- The first version key used a 32-digit numeric string and overflowed BusyBox
  integer comparison; it was reduced to a bounded 12-digit key and strict
  version regex.
- The first rollback path ignored package restore failure and could report
  `rolled_back` falsely; it now restarts and health-checks the restored package
  and reports `rollback_failed` when recovery is incomplete.
- Release package control metadata is not assumed to support arbitrary custom
  control fields; the package also ships a compatibility file, and the updater
  validates that fallback.
- Rollback now passes `--force-downgrade` to opkg and the test stub parses
  option-bearing invocations, matching OpenWrt downgrade behavior.
- Validation and low-space failure paths now remove their temporary unpack
  directories; the shell regression test asserts that no check workspace is
  leaked.
- Rollback failures retain the transaction for a later `recover`; stale PID
  locks from hard power loss are reclaimed only when the owner is dead.
- Both persistent state and `/tmp` free space are checked, commit-copy errors
  trigger rollback, and state/transaction files are root-only.
- Update packages require a signed sidecar manifest with outer-package,
  control-archive, and data-archive SHA-256 values. The builder emits the
  manifest and release signing is supplied through `WFC_UPDATE_SIGNING_KEY`.
- `ax6s-existing` now packages the V2 helper/supervisor adapter and has an
  actual build-and-unpack regression test.
- The update-manifest and AX6S package tests accept both canonical GNU tar
  member names and `./`-prefixed BSD tar names; the existing-package test also
  verifies the support-bundle helper.
- GitHub run `32471646456` exposed a pre-existing nondeterministic sing-box
  stderr fixture; it now emits one deterministic diagnostic and `exec`s a
  long-lived process, with five repeated local runs passing.
- Commit hooks report `lefthook` unavailable in PATH; repository verification
  completed successfully.

## Warnings and non-blocking notes

- Existing cargo audit duplicate `socket2` and `windows-sys` lock entries remain
  warnings; advisories, bans, licenses, and sources pass when the cached
  advisory database is used. The final local online audit refresh was blocked
  by a transient RustSec TLS error.
- The LuCI surface expects the operator to stage the IPK locally under
  `/tmp/wloc-update`; it intentionally does not fetch arbitrary URLs or upload
  packages. Hardware flash-full and hard-power-cut evidence is Issue #40.
- The updater requires a known-good rollback IPK. First installation must
  establish `current.ipk` before transactional upgrades are enabled.

## Unresolved decisions and blockers

- AX6S memory/storage/CPU measurement and real-device update/rollback evidence:
  Issue #40.
- Full migration rehearsal, release matrix, and V2 publication: Issue #41.

## Capabilities required for the next Agent

- `openwrt`, `test`, `security`, `docs`, and independent review of root-only
  package and rollback boundaries.

## Environment assumptions

- `opkg`, `tar`, `df`, `grep -E`, `sed`, and `awk` are available in the target
  OpenWrt image.
- `/tmp/wloc-update` is writable for explicit local staging; persistent state
  under `/var/lib/wificalling-location-gateway/update` is writable by root.
- The unified supervisor is the only permitted lifecycle restart boundary.

## Security and privacy notes

- Package input is local-only, path-bounded by rpcd, archive-traversal checked,
  and never streamed from an untrusted URL.
- No credentials, CA private keys, raw traffic, precise locations, or device
  identifiers are included in transaction status or error output.
- Update operations never manipulate the stable Gateway nftables namespace or
  UDP 500/4500 handling; failed activation remains passthrough via supervisor
  rollback.

## Next executable steps

1. Run `scripts/agent-handoff.sh 39 codex-v2-lead openwrt,test,security,docs`.
2. Push the branch and open a PR with `Closes #39`, this handoff capsule, test
   evidence, rollback notes, and known hardware-test gap.
3. Require independent security/test review and all GitHub checks before merge.
4. After merge, take Issue #40 for AX6S resource profiling and real-device
   upgrade/rollback evidence.
