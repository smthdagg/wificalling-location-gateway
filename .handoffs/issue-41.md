# Agent handoff: Issue 41

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: integration,security,test,openwrt,ui
- Branch: codex/issue-41-v2-release-codex-v2-lead-20260821133002-404596fe
- Checkpoint parent: 501dea9
- Updated at (UTC): 2026-08-21T20:41:13Z
- Credentials included: no

## Objective

Complete V2.0 standalone WLOC service, multi-device profiles, LuCI
management/diagnostics/update surfaces, low-resource AX6S packaging, migration,
rollback, documentation and release acceptance without claiming hardware gates
that have not been observed.

## Completed

- Unified supervisor owns the standalone WLOC/provider lifecycle and LuCI
  restart paths; it does not read or manage another Gateway project.
- Profile CRUD is UCI-backed with validation, commit/revert behavior and
  per-profile status/log diagnostics.
- WLOC redirect state is scoped to the assigned device, exact Apple hosts and
  TCP 443; stop and provider failure withdraw the WLOC table.
- Raw WLOC/debug output is opt-in, bounded and privacy-safe; support bundles
  redact profile/device/location material.
- Added shared sing-box provider resolution for AX6S-tested tiny/lite or
  PassWall binaries without packaging a duplicate full-size binary.
- Updated authoritative development, deployment, package, release, migration
  and rollback documentation to require removing old application packages
  before AX6S installation while retaining the selected provider.
- Added package and migration contract tests and integrated them into CI.
- Final AX6S evidence is recorded in
  `docs/testing/AX6S_REAL_DEVICE_2026-08-22.md`, including successful
  2.0.0-17 -> 2.0.0-18 update, health-failure rollback 2.0.0-19 ->
  2.0.0-18, and the final remove/install/restart test of release candidate
  2.0.0-1. The actual mobileconfig export also passed with no temporary CA
  file left behind; publication and independent review remain external
  release actions.
- Release packaging now emits and verifies three assets: AX6S AArch64 IPK,
  OpenWrt 24.x x86_64 IPK, and OpenWrt 25.x APK. The four-case Docker matrix
  passed with socket/status checks.
- The V2 LuCI source and package copy are identical for the device page and
  translation map. All current `i18n.t()` keys have Chinese entries in the
  mirrored PO catalogue and formal `po/zh_Hans` source; a test rejects stale
  Gateway-only UI text.
- The final rebuilt package was installed again on AX6S after removing the old
  package. Its final hash is
  `90762e2453ffae11341fef6caa42bef379ba52a9599d5aeb73bcc0a2952f231f`; the
  exact `mediatek/mt7622` target, API, UI/PO files, mobileconfig export,
  service health, RSS, storage and memory checks passed.
- The component updater now rejects a package whose `X-WLOC-Target` differs
  from the router's `DISTRIB_TARGET`; RED/GREEN commits are `1172a03` and
  `b68ff9d`.
- The independent feed repository was refreshed locally with the current three
  release packages, SHA256SUMS, and signed/verified `Packages`/
  `Packages.gz`. Its standalone V2 README was pushed to the feed `main`
  branch (`a264a25`); gh-pages package publication remains after the main PR
  merge and the explicitly open real-client/physical-fault gates.

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
| `./scripts/ci/verify.sh` | Passed | Rust, Python, JavaScript, package/UI/shell/security/resource/update checks |
| Rust coverage | Passed | 80.40% line coverage, above the 80% gate |
| `tests/scripts/test-singbox-runtime.sh` | Passed | provider selection, version validation and explicit invalid-provider rejection |
| `tests/scripts/test-ax6s-migration-contract.sh` | Passed | backup, stop/disable, remove-only-old-apps, post-removal space check and install ordering |
| `tests/scripts/test-standalone-ax6s-package.sh` | Passed | architecture-specific package layout and provider warning |
| `python3 -m unittest discover -s tests -p 'test_v2_ui_contract.py'` | Passed | 9 UI/catalogue contract tests |
| `./scripts/openwrt/verify-docker-matrix.sh --dist-dir dist/openwrt-release` | Passed | 4 OpenWrt/iStoreOS package installs and status checks |
| `gh pr checks 60 --repo smthdagg/wificalling-location-gateway` | Passed | openwrt-cross-build, verify, and pull-request-contract |
| feed SHA256/signature preflight | Passed | three package hashes and both usign signatures verified locally |
| `./tests/scripts/test-structured-logs.sh` | Passed | GNU tar-compatible support-bundle member lookup |
| `git diff --check` | Passed | no whitespace errors |

## Failed attempts

- No current test failure remains; the findings below were fixed during the
  final verification loop.

## Findings fixed during final verification

- The standalone supervisor initially lost its executable bit in the package;
  package tests now assert it.
- One device was incorrectly placed behind the multi-device readiness barrier;
  readiness now requires more than one device profile.
- A volatile PassWall `/tmp` config path caused provider failure after reboot;
  AX6S uses `/var/etc/passwall` and the supervisor validates the provider file.
- Docker image digest syntax in the OpenWrt cross-build helper was corrected;
  final AArch64 binaries were rebuilt with the pinned toolchain.
- The component updater now ignores `all`/`noarch` architecture stanzas and
  requires the WLOC redirect table/rule in its health gate before committing.
- The release builder was corrected to include the AX6S package in the
  three-package checksum/matrix set and to avoid sending APK v3 through the
  IPK manifest parser.
- The shared LuCI translation map no longer carries the old Wi-Fi Calling-only
  settings/monitor strings; the historical combined tutorials now point users
  to the standalone WLOC v2 guide.
- Mobileconfig export now uses a unique mode-0600 temporary file under `/tmp`
  with cleanup traps instead of the fixed `/tmp/wloc-ca.b64` path.
- The device page was synchronized between canonical and package sources; all
  current UI keys were added to the formal Chinese PO catalogue, and the stale
  `restart the gateway` translation was removed from the standalone product.
- GitHub Linux verification exposed a portability bug in the structured-log
  test: GNU tar does not match the BSD-style `./wloc-support/...` lookup. The
  test now uses the archive's canonical `wloc-support/events.jsonl` member and
  the complete local gate passes again.

## Final acceptance status

- AX6S standalone runtime, migration, provider reuse, resource, fail-open,
  profile CRUD, manual/auto location persistence, stop/start, reboot, signed
  update, transactional health-rollback, final release-candidate install,
  UI/PO assets, and mobileconfig export gates:
  **pass**.
- Real iPhone WLOC traffic was not run because no client fixture was supplied.
- Hard power-cut during opkg, flash-full injection, and real iPhone WLOC
  traffic were not run. Signed feed publication, PR review, merge, tag, and
  release publication are not performed by this local execution.

## Next executable steps

1. Review the standalone diff and AX6S evidence with an independent reviewer.
2. Run the supplied client fixture, if available, for real WLOC traffic and
   isolation evidence.
3. Sign the generated package manifests with the protected release key and
   perform the normal PR/merge/tag/release workflow with explicit approval.

## Capabilities required for the next Agent

- openwrt,test,security,integration,ui

## Environment assumptions

- The branch is based on the project handoff commit recorded in Git history;
  no PR number is asserted here because external publication was not executed.
- The AX6S test device is on a compatible 24.x OpenWrt/ImmortalWrt image and
  has a tested sing-box tiny/lite or PassWall provider available.
- Host-side tests do not emulate procd, flash exhaustion, power loss, or live
  iPhone traffic.

## Security and privacy notes

- No API keys, tokens, private keys, `.env` values, raw captures, device
  identifiers, precise locations or production traffic are included.
- The provider resolver validates absolute executable paths and the supervisor
  validates the configured provider file with `sing-box check`; it never
  attaches to or mutates a PassWall-owned process/config.
- WLOC failures withdraw redirect/return to passthrough and must not create a
  default fake coordinate.
