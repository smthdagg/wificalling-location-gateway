# Agent handoff: Issue 64

## Identity and scope

- Source agent ID: codex-issue64-missing-node
- Capabilities used: rust,openwrt,ax6s,ci,test,docs,release
- Branch: codex/issue-64-missing-node-fail-closed
- Stable base commit: `3012be7`
- Updated at (UTC): 2026-08-23
- Credentials included: no

## Objective

Prevent WLOC auto-follow from selecting an unrelated node after the followed
WCG device binding is deleted, make refresh results visible, enforce the stable
integrated 1.2.x release boundary, and produce/test R4 packages.

## Completed

- Removed the first-outbound/first-endpoint fallback from the authoritative UCI
  path and rejected stale generated routes when the device binding is absent.
- Added `BoundNodeMissing`, cleared stale exit/Geo state, and exposed an
  actionable monitor error with English and Chinese UI text.
- Made the LuCI refresh action await status refresh and report changed,
  unchanged, or unavailable exit state.
- Restricted both release builders to hash-pinned stable integrated 1.2.x
  package inputs and documented the independent Beta project boundary.
- Built all three R4 package assets, passed the four-environment Docker matrix,
  and installed the exact AArch64 asset on AX6S using the low-storage workflow.
- Passed the temporary missing-binding fail-closed test and restored the exact
  original router configuration afterward.

## Verification

| Gate | Result |
|---|---|
| Rust deleted-node tests | Passed |
| WLOC app/status integration tests | Passed |
| All LuCI JavaScript tests | Passed |
| Package baseline rejection tests | Passed |
| `./scripts/ci/verify.sh` | Passed; 69 Python tests, all Rust/JS suites, 81.34% line coverage, audit/license/secret/repository gates |
| Four-environment Docker matrix | All installed, started, socket-ok, status-ok |
| AX6S package/runtime | R4 installed; WCG/WLOC/config/socket healthy |
| AX6S missing binding | Exit and Geo unavailable; no fallback; restore returned verified |

Release hashes are recorded in
`docs/testing/V1.2.2_R4_DELETED_NODE_HOTFIX.tdd.md` and the generated release
`SHA256SUMS` manifest.

## Failed attempts

- The first AArch64 build found the pinned Rust image absent locally. Pulling
  the exact digest specified by the repository restored the reproducible build.
- The first x86 package output directory name did not satisfy the builder's
  dedicated-directory safety rule. A correctly named dedicated directory was
  used; no package was emitted by the rejected attempt.
- An initial router status command used exact-name `pgrep`, which does not work
  reliably with OpenWrt process-name truncation. `pidof` and service state were
  used for the final evidence.

## Next executable steps

1. Run the final repository verification after documentation changes.
2. Push this branch and open a stacked PR after the stable-tree restoration PR.
3. Obtain independent review before merging or publishing the R4 GitHub release.
4. Re-download any published assets and verify them against `SHA256SUMS`.

## Capabilities required for the next Agent

- rust
- openwrt
- ci
- release
- security-review
- stable/Beta repository separation

## Security and privacy notes

- No credentials, node secrets, CA private keys, precise locations, or raw
  device traffic are included.
- The test changed only the existing test device's node reference, under an
  automatic restore trap, and verified the original UCI file afterward.
- Interception scope and UDP 500/4500 behavior were not broadened.
