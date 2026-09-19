# Install and startup hardening evidence

This record covers the failure mode where a clean installation appears to
complete but WLOC or the Gateway needs manual Agent intervention before it is
usable.

## User journeys

| Journey | Expected result | Evidence |
|---|---|---|
| Fresh install without a configured device or node | WLOC remains pass-through and its control socket stays available for LuCI configuration | `tests/scripts/test-wloc-runtime-contract.sh` |
| Package restart fails during post-install | Installation reports failure instead of silently succeeding | `tests/scripts/test-package-variants.sh` |
| Router DNS differs from public resolvers or a resolver is filtered | WLOC retries the router-provided resolver list before public fallbacks | `tests/scripts/test-wloc-runtime-contract.sh` |

## TDD checkpoints

- RED: the clean-install contract failed because WLOC defaulted to enabled,
  and package post-install scripts swallowed restart failures.
- GREEN: WLOC now starts in pass-through when its scope is not ready, package
  restart failures are visible, and DNS resolution retries active resolvers.
- Repository verification: `./scripts/ci/verify.sh` passed; Rust line coverage
  was 81.51%.

## Remaining release evidence

The contract tests do not replace a real-device release matrix. Before calling
the next package public/stable, run fresh install, upgrade with preserved
configuration, rollback, filtered/IPv6 DNS, PassWall coexistence, reboot and
low-memory recovery, plus a real two-SIM VoWiFi test on AX6S. The current AX6S
runtime fixes are still direct hotfixes on package `1.3.0-r16`; they must be
rebuilt and shipped as the next package revision before release.
