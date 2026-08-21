# Agent handoff: Issue 37

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: openwrt,luci,ui,test
- Branch: codex/issue-37-v2-luci-management-codex-v2-lead-20260821084324-791a3cb5
- Checkpoint parent: 5844933
- Final local checkpoint: de09ae1
- Credentials included: no

## Objective

Implement the first V2 unified LuCI management surface: basic WLOC service
settings, bounded multi-device profiles, node selection, auto/manual WLOC
mode, per-profile enablement, redacted live Gateway/WLOC/profile state, and a
single safe Apply & restart boundary suitable for small OpenWrt gateways.

## Completed

- Added a unified Basic settings + Device profiles page in both OpenWrt source
  trees.
- Changed profile edits, additions, and deletions to stage in LuCI UCI state;
  only Apply & restart calls `uci.save()`, `ui.changes.apply(true)`, and the
  unified supervisor restart.
- Added bounded validation for profile count, IDs, labels, node references,
  private IPv4/unicast MAC device bindings, duplicate normalized bindings,
  location modes, coordinates, probe interval, and Geo provider.
- Added Gateway/WLOC health summary plus per-profile `phase (reason_code)`;
  refresh is one bounded request per 15 seconds and updates state cells without
  rebuilding staged inputs.
- Added write-only ACL placement for `restart_unified`; canonical/package ACL
  sources are now byte-identical and tested as such.
- Added TDD contract tests and `docs/testing/V2_LUCI_MANAGEMENT.tdd.md`.
- Added post-apply health refresh fallback so a transient health RPC failure
  does not falsely report a successful configuration apply as failed.

## Commits

- `6c0f721 test: define unified LuCI settings contract`
- `8f00ebc feat: add unified LuCI settings and profile apply boundary`
- `4230d44 fix: tolerate health refresh failures after apply`
- `de09ae1 fix: tighten profile address validation and LuCI ACLs`

## Files changed

- `openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js`
- `openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js`
- `openwrt/files/usr/share/rpcd/acl.d/luci-app-wificalling-location-gateway.json`
- `openwrt/luci-app-wificalling-location-gateway/files/usr/share/rpcd/acl.d/luci-app-wificalling-location-gateway.json`
- `tests/test_v2_ui_contract.py`
- `docs/testing/V2_LUCI_MANAGEMENT.tdd.md`

## Verification

| Command | Result | Evidence |
|---|---|---|
| `./scripts/ci/verify.sh` | Passed | 74 Python tests, all Rust targets, OpenWrt/AX6S/release packaging, JS/ACL checks, secret scan, cargo audit |
| `cargo llvm-cov --workspace --all-targets --locked --fail-under-lines 80` | Passed | 80.17% total Rust line coverage |
| `python3 -m unittest tests.test_v2_ui_contract` | Passed | 5 UI/ACL contract tests |
| `node --check` on both `wloc-devices.js` copies | Passed | Both LuCI sources parse |
| `git diff --check` | Passed | No whitespace errors |
| Independent reviewer Raman | APPROVE after fixes | Two P2 findings fixed: address validation and ACL drift/read permission |

## Failed attempts

- The initial GREEN implementation accepted arbitrary non-empty device
  strings; independent review caught this before PR and the UI now mirrors the
  private IPv4/unicast MAC boundary.
- The first ACL copies drifted and exposed `restart_unified` in read; both
  sources were aligned and the write-only contract is now tested.
- A post-apply health RPC rejection could misreport a successful apply; the
  refresh is now defaulted and non-fatal.

## Warnings and non-blocking notes

- Cargo audit reports existing duplicate `socket2` and `windows-sys` lockfile
  entries; advisories, bans, licenses, and sources all pass.
- The two LuCI page copies are functionally aligned but one remains formatted
  and the package mirror is compact; the ACL copies are byte-identical. A
  future packaging cleanup can make page source synchronization generated
  rather than manually maintained.
- LuCI client-side validation protects the apply path; a future backend apply
  RPC can make commit/restart rollback atomic across daemon failures. This is
  documented rather than presented as already transactional at the daemon
  boundary.

## Unresolved decisions and blockers

- Structured bounded logs/support bundle: Issue #38.
- Component update, compatibility gate, and rollback: Issue #39.
- AX6S memory/storage/CPU gates and real-device evidence: Issue #40.
- Migration rehearsal, complete integration acceptance, and V2 release: Issue #41.
- Gateway settings remain on the existing WFC page while this issue establishes
  the unified WLOC/profile management foundation; the broader one-service UI
  consolidation is part of the remaining V2 work.

## Capabilities required for the next Agent

- `openwrt`, `luci`, `ui`, `test`, and security-sensitive ACL/procd review.

## Environment assumptions

- LuCI provides UCI staging, `ui.changes.apply(true)`, rpcd ACL separation,
  `poll`, and the standard `E()` DOM helper.
- The runtime profile redirect adapter currently installs IPv4-scoped routes;
  the UI therefore accepts private IPv4 or unicast MAC bindings, while MAC
  profiles remain explicitly degraded until a runtime address resolver exists.
- The two OpenWrt source trees are packaged separately and must remain covered
  by mirrored contract tests.

## Security and privacy notes

- No credentials, tokens, private keys, raw traffic, or precise runtime Geo
  values are displayed by the new page.
- Runtime scope remains assigned device + exact Apple WLOC hostnames + TCP 443;
  UDP 500/4500 and the stable Gateway nftables namespace are untouched.
- Invalid addresses, duplicate bindings, unknown modes, failed health checks,
  and restart errors fail closed/open according to the existing runtime
  contract; the LuCI page never invents a location.

## Next executable steps

1. Run `scripts/agent-handoff.sh 37 codex-v2-lead openwrt,luci,ui,test`.
2. Open a PR against `main` with `Closes #37`, this handoff capsule, and the
   full verification evidence.
3. Require GitHub checks and independent review before merge.
4. After merge, take Issue #38 for bounded structured logs and support bundle.
