# Agent handoff: Issue 87

## Identity and scope

- Source agent ID: zcode-preset-autosave-r12-20260829
- Capabilities used: test,ci,docs
- Branch: codex/issue-87-r12-preset-autosave
- Checkpoint parent: `492dc72` (v1.3.0-r11)
- Updated at (UTC): 2026-08-29
- Credentials included: no

## Objective

Publish v1.3.0-r12 adding the requested preset auto-save: applying a manual
location (search result or raw coordinates) automatically upserts an entry in
the saved-locations table so it can be re-applied or deleted with one tap.

## Completed

- `wloc.js`: the Apply-coordinates success path upserts a `config preset`
  section — a search result is stored under its city label (tracked as
  `lastSearch` and only used when the applied coordinates match it), raw
  coordinates under "lat, lon"; dedupe prefers an exact coordinate match and
  then an exact label match, so re-applying a place updates the entry instead
  of duplicating it.
- The saved-locations table refreshes immediately after apply
  (`renderPresets()`), with a bilingual confirmation message and a new i18n
  key.
- Package release metadata bumped to `1.3.0-r12` with bilingual README,
  changelog, and release-test updates; no daemon or protocol changes.

## Verification

- All seven `tests/js/*.test.js` suites pass (the edited handler sits next to
  the mode-switch atomicity assertions).
- Full `./scripts/ci/verify.sh` gate green.

## Failed attempts

- None; the first patch applied cleanly.

## Next executable steps

- Merge the release PR after CI is green.
- Build the six Standard/Lite assets (Rust runtimes reused from the r10/r11
  build — no Rust change), run the Docker install matrix, update and re-sign
  the feed.
- Tag `v1.3.0-r12`, publish the release, and run the live AX6S r11-to-r12
  upgrade.

## Capabilities required for the next Agent

- GitHub CLI (`gh`) with write access to `smthdagg/wificalling-location-gateway`
  and `smthdagg/wificalling-location-gateway-feed`.
- Docker for feed signing and the install matrix.
- SSH access to the AX6S test router (`192.168.31.1`, root) for the live
  upgrade validation.

## Security and privacy notes

- No credentials are included in this capsule or in the repository.
- Presets remain plain UCI sections on the router; no location data leaves the
  device beyond the existing search flow.
