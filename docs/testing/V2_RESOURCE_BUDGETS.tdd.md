# V2-08 Resource budgets and AX6S evidence

## Scope

This contract covers small-memory/small-storage OpenWrt gateways. It separates
offline CI gates from measurements that require an actual AX6S staging device.
No real device identifier, address, credential, raw traffic, or precise
location belongs in the evidence.

## User journeys

- As a gateway maintainer, I want binary and package size regressions rejected
  in CI, so a low-storage device is not silently overfilled.
- As an operator, I want a bounded resource report for startup, RSS, CPU,
  logs, caches, profiles, restart, and rollback, so I can tell whether the
  service is safe to keep enabled.
- As a release owner, I want a repeatable AX6S checklist, so real-device
  acceptance does not depend on undocumented observations.

## Machine-readable contract

The installed contract is
`/usr/share/wificalling-location-gateway/resource-budget.conf`.

| Budget | Ceiling | Enforced by |
|---|---:|---|
| Runtime binaries combined | 8 MiB | `verify-resource-budgets.sh` |
| Integrated package | 20 MiB | `verify-resource-budgets.sh` when artifact is supplied |
| Persistent update/config state | 10 MiB | package contract and device checklist |
| Logs combined | 1 MiB | package contract; individual WLOC log is 64 KiB |
| Caches combined | 1 MiB | package contract and device checklist |
| Profiles | 8 | existing profile model and package contract |
| Startup | 10 s | resource report gate when supplied |
| Peak RSS | 35 MiB | resource report gate when supplied |
| Probe CPU | 30% | resource report gate when supplied |

The existing 64 KiB WLOC event log, bounded response/cache sizes, 8-profile
model, and bounded restart policy remain stricter component-level limits.

## TDD evidence

| Stage | Command | Result |
|---|---|---|
| RED | `sh tests/scripts/test-resource-budgets.sh` before the contract/harness existed | PASSING RED condition: missing budget/harness caused exit 1 |
| GREEN | `sh tests/scripts/test-resource-budgets.sh` | PASS; report/package schema, procfs path where available, and oversized binary/package/resource rejection exercised |
| GREEN | `cargo build --locked --release --bins && ./scripts/ci/verify-resource-budgets.sh` | PASS; combined runtime binaries: 2,892,256 bytes |
| Syntax | `sh -n scripts/ci/*.sh tests/scripts/test-resource-budgets.sh` | PASS |

The total runtime size is a local measurement of the current release build,
not AX6S RSS evidence. Idle RSS remains an AX6S observation recorded in the
evidence template; the portable gate enforces peak RSS because it is the
portable upper-bound signal available across host and target profilers. The
complete repository gate runs this contract after building all release
binaries. The release package builder invokes `verify-package-budget.sh` for
each generated IPK/APK, so package size is checked against the same installed
contract when a real package exists.

## AX6S acceptance gap

Hardware acceptance is intentionally not claimed in this change until an
actual staging AX6S is available. Fill
`docs/testing/AX6S_RESOURCE_EVIDENCE.template.md` using the target-side
`scripts/ci/profile-resource.sh` (which has a `/proc` fallback and does not
require GNU time or Python 3) or an equivalent BusyBox-compatible capture,
then attach only redacted aggregate values to the Issue/PR.
