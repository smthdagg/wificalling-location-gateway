# Agent handoff: Issue 40

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: openwrt,test
- Branch: codex/issue-40-ax6s-resource-gates-review-codex-v2-lead-20260821114718-3db18a06
- Final local checkpoint before handoff: pending independent review, CI, and AX6S hardware evidence
- Credentials included: no

## Objective

Define and enforce low-memory/low-storage resource budgets for the unified
Gateway/WLOC component, with reproducible host-side gates and a redacted AX6S
measurement template. Do not claim real-device measurements from the host.

## Completed

- Added versioned resource metadata to both canonical OpenWrt package source
  trees: combined release binary, integrated package, persistent state, log,
  cache, profile count, startup, RSS, and CPU ceilings.
- Added portable `profile-resource.sh` plus Python and procfs fallbacks. Reports
  contain only bounded status, elapsed time, peak RSS, CPU, and command status
  fields.
- Added `verify-resource-budgets.sh` to reject missing/irregular artifacts,
  oversized runtime binaries, failed commands, and out-of-budget resource
  reports; added `verify-package-budget.sh` for each actual IPK/APK output.
- Installed the budget metadata through the canonical OpenWrt Makefile and
  documented the contract in the OpenWrt package README and development test
  plan.
- Added host-side RED/GREEN TDD evidence and a redacted AX6S evidence template
  covering disabled/one/multiple/degraded/restart/update/rollback/low-space
  scenarios, including the existing safety invariants.
- Added resource checks to the normal repository verification path and changed
  release compilation to build all runtime binaries before the size gate.
- Added package artifact and resource-report assertions to the regression test;
  oversized runtime/package artifacts, failed reports, RSS, CPU, and startup
  reports are rejected.

## Files changed

- `openwrt/files/usr/share/wificalling-location-gateway/resource-budget.conf`
- `openwrt/luci-app-wificalling-location-gateway/files/usr/share/wificalling-location-gateway/resource-budget.conf`
- `openwrt/Makefile`
- `openwrt/README.md`
- `scripts/ci/profile-resource.py`
- `scripts/ci/profile-resource.sh`
- `scripts/ci/verify-resource-budgets.sh`
- `scripts/ci/verify-package-budget.sh`
- `scripts/ci/verify-rust.sh`
- `scripts/ci/verify.sh`
- `scripts/openwrt/build-release-packages.sh`
- `tests/scripts/resource-fixture.sh`
- `tests/scripts/test-resource-budgets.sh`
- `DEVELOPMENT_TEST_PLAN.md`
- `docs/testing/V2_RESOURCE_BUDGETS.tdd.md`
- `docs/testing/AX6S_RESOURCE_EVIDENCE.template.md`

## Verification

| Command | Result | Evidence |
|---|---|---|
| `sh tests/scripts/test-resource-budgets.sh` before implementation | Passed as RED checkpoint | Commit `5453ec7` exited non-zero because the resource contract/harness was absent |
| `sh tests/scripts/test-resource-budgets.sh` | Passed | Budget schema, portable profiler, package/report checks, and oversized artifact rejection |
| `./scripts/ci/verify.sh` | Passed | 80 Python tests, all Rust targets, secret scan, packaging/update tests, resource tests, audit, release build |
| Rust coverage | Passed | 80.11% line coverage, above the 80% repository gate |
| Rust audit | Passed | Advisories, bans, licenses, sources all pass; existing duplicate `socket2`/`windows-sys` warnings remain |
| Release resource gate | Passed | Combined runtime binaries `2,958,912` bytes against `8,388,608` bytes |
| `git diff --check` | Passed | No whitespace errors |
| ShellCheck | Pending CI | Not installed in the local environment |

## Failed attempts

- The first resource regression test printed its success line twice; the
  duplicate assertion was removed.
- Running the resource gate against the fixture's measured CPU made the test
  host's transient 72% CPU sample fail the product budget. The test now keeps
  profiler format coverage and feeds a controlled compliant report to the
  gate; product CPU acceptance remains an AX6S hardware measurement.
- The handoff checker initially rejected the title `TDD and verification`;
  it was renamed to the required `Verification` heading.
- The first resource design declared an idle RSS hard ceiling without a
  portable collector; the unimplemented key was removed from the machine
  contract and idle RSS is now explicitly a redacted AX6S observation.
- Package size was initially only optional in the generic runtime gate; a
  dedicated package gate is now invoked for every release IPK/APK.
- The profiler initially required GNU time or Python 3 on target; a lightweight
  `/proc` sampler now covers small Linux/OpenWrt images.
- The local commit hook reports `lefthook` unavailable in PATH; repository
  verification itself passed.

## Known limitations and acceptance gap

- The host cannot provide AX6S hardware evidence. RSS, CPU, startup, flash
  usage, restart/reload, update/rollback, and low-space behavior must still be
  measured on the actual staging router using the redacted template before
  claiming V2-08 complete or enabling a persistent release rollout.
- CI enforces deterministic runtime artifact and report gates, while the
  release builder enforces each real package artifact. It intentionally
  does not pretend to run the full OpenWrt service, TProxy/nftables path, or
  real update transaction on the host.
- The selected budgets are release defaults for a small gateway and require
  project-lead acceptance or a recorded exception if AX6S measurements show a
  different safe ceiling.

## Capabilities required for the next Agent

- `openwrt`, `test`, `security`, and access to an AX6S staging router for the
  final resource and update/rollback evidence.

## Safety invariants retained

- Fail-open behavior remains the release contract.
- WLOC interception remains limited to the assigned device, exact approved
  Apple hosts, and TCP 443.
- UDP 500/4500 and the stable Gateway nftables table are untouched.
- Reports and evidence must not contain credentials, CA keys, raw traffic,
  device identifiers, or precise user location.

## Security and privacy notes

- Resource reports are allowlisted and mode-restricted; the profiler suppresses
  command output and rejects symlink report paths.
- Package and runtime gates inspect regular local files only and do not fetch or
  upload artifacts.
- AX6S evidence must be redacted before entering Git or GitHub; no credentials,
  CA private keys, raw traffic, or device identifiers are included.

## Next executable steps

1. Push the branch and open a review PR for Issue 40 without claiming AX6S
   hardware acceptance.
2. Obtain independent review focused on resource-gate bypasses and OpenWrt
   portability; resolve P1/P2 findings.
3. Run GitHub CI, including ShellCheck and the pinned OpenWrt cross-build.
4. On an actual AX6S staging router, fill the redacted evidence template and
   record before/after install, steady state, reload/crash, rollback, and
   low-space results. Only then decide whether to merge/close the full Issue.
5. Carry the accepted budgets into Issue 41's release matrix and migration
   rehearsal.
