# Agent handoff: Issue 81

## Identity and scope

- Source agent ID: zcode-luci-errorpaths-r11-20260829
- Capabilities used: test,ci,docs
- Branch: codex/issue-81-r11-luci-error-paths
- Checkpoint parent: `0e24f38` (v1.3.0-r10)
- Updated at (UTC): 2026-08-29
- Credentials included: no

## Objective

Publish v1.3.0-r11 with the LuCI error-path fixes: rpc rejections in the
location settings view left controls permanently stalled (Enable/Disable
switch silently desynced from the daemon, Search and Apply-coordinates
buttons disabled until a page reload), which users experience as a stuck
dialog or a frozen page whenever the daemon is busy or errors.

## Completed

- `wloc.js`: the Enable/Disable switch chain now handles rejection — it shows
  a visible error notification and flips the checkbox back so the UI cannot
  silently disagree with the real service state.
- `wloc.js`: the Search and Apply-coordinates chains re-enable their buttons
  in a rejection handler instead of leaving them disabled until a reload.
- Package release metadata bumped to `1.3.0-r11` with bilingual README,
  changelog, and release-test updates; no daemon or protocol changes.

## Verification

- All seven `tests/js/*.test.js` suites pass, including the mode-switch
  atomicity and monitor-refresh regressions (the edited chains are adjacent to
  their assertions).
- Full `./scripts/ci/verify.sh` gate green (Python, packaging, secret scan,
  cargo audit/deny, ShellCheck via CI image).

## Failed attempts

- None; the first patch applied cleanly and the JS suites passed on the first
  run.

## Next executable steps

- Merge the release PR after CI is green.
- Rebuild the six Standard/Lite assets (Rust runtimes are unchanged and are
  reused from the r10 build), run the Docker install matrix, update and
  re-sign the feed.
- Tag `v1.3.0-r11`, publish the release, and run the live AX6S r10-to-r11
  upgrade.

## Capabilities required for the next Agent

- GitHub CLI (`gh`) with write access to `smthdagg/wificalling-location-gateway`
  and `smthdagg/wificalling-location-gateway-feed`.
- Docker for feed signing and the install matrix.
- SSH access to the AX6S test router (`192.168.31.1`, root) for the live
  upgrade validation.

## Security and privacy notes

- No credentials are included in this capsule or in the repository.
- No daemon, protocol, or data-handling changes; the release only ships fixed
  LuCI views and version metadata.
