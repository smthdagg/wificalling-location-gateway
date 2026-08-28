# Agent handoff: Issue 72

## Identity and scope

- Source agent ID: codex-node-status
- Capabilities used: openwrt,test,ci,integration
- Branch: codex/issue-72-node-status-manual-refresh
- Checkpoint parent: `7a8781d`
- Updated at (UTC): 2026-08-28
- Credentials included: no

## Objective

Refresh a node row's status, Ping/latency, and quality immediately after a
manual LuCI `nodeTest` result.

## Completed

- Added regression guards for manual row refresh and polling persistence.
- Updated standalone and integrated LuCI views to normalize and render manual
  TCP and WireGuard results in the existing three metric cells.
- Kept the manual result visible for 60 seconds so an older status export
  cannot immediately overwrite it.
- Reused the single running WIFICalling Gateway sing-box for all node quality
  probes through dedicated loopback HTTP inbounds; no temporary process or
  duplicate proxy configuration is created.
- Published monitor results to `/www/wloc-node-status.json` and added browser
  cache bypassing so background metrics are visible in LuCI.
- Bumped the package release to `1.3.0-r7` and updated bilingual README,
  changelog, packaging metadata, tests, and release documentation.
- Built all six Standard/Lite assets for the three supported platform targets
  and installed r7 Lite on AX6S.

## Files changed

- Three LuCI overview sources under `openwrt/`.
- `tests/js/wg_handshake_reason.test.js` and release tests.
- Release metadata, README/changelog, and the TDD evidence document.

## Verification

| Command | Result | Evidence |
|---|---|---|
| `node tests/js/wg_handshake_reason.test.js` | Passed | Manual row refresh and 60-second polling overlay guards |
| `./scripts/ci/verify.sh` | Passed | 69 Python tests, Rust suites, 81.35% line coverage, audits and repository gates |
| OpenWrt/iStoreOS matrix | Passed | 8/8 Standard/Lite rows installed, started, socket-ok, status-ok |
| AX6S r7 Lite installation | Passed | Five node metrics, one WIFICalling sing-box process, status polling, and 30-second stability verified |
| Release assets and checksums | Passed | Six r7 packages built and `SHA256SUMS` verified |
| `git diff --check` and secret scan | Passed | No whitespace errors or sensitive values in the change |

## Failed attempts

- The first AX6S package build omitted the required binary environment inputs;
  the build was rerun with the existing verified runtime binaries and passed.

## Unresolved decisions and blockers

- None.

## Next executable steps

1. Keep PR #74 linked to this capsule and Issue #72.
2. Wait for all GitHub repository gates and merge after they pass.

## Capabilities required for the next Agent

- openwrt
- test
- ci
- security-review

## Environment assumptions

- OpenWrt 24.10 uses IPK and OpenWrt 25.12 uses native APK.
- Existing UCI configuration is preserved during package replacement.

## Security and privacy notes

- No API keys, tokens, private keys, `.env` values, raw captures, device
  identifiers, or precise user locations are included.
- The manual result is a display overlay only; it does not alter routing or
  the backend status producer.
