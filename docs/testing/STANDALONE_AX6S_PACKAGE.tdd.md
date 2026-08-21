# AX6S standalone WLOC package builder — TDD evidence

## User journey

As an AX6S administrator, I can install one architecture-specific WLOC IPK
without installing or depending on a Wi-Fi Calling Gateway and without losing
the existing WLOC configuration or CA.

## RED

- Command: `./tests/scripts/test-standalone-ax6s-package.sh`
- Result before implementation: `unsupported dependency mode: ax6s-standalone`
- A second boundary test then failed because the first implementation incorrectly retained an `all` architecture filename and metadata while carrying AArch64 executables.
- The naming refinement produced RED with `FAIL: standalone package filename must use the project name and identify the AX6S architecture` before the complete product package was renamed.

## GREEN

- Command: `./tests/scripts/test-standalone-ax6s-package.sh`
- Result: `standalone AX6S package tests passed`
- Full command: `./scripts/ci/verify.sh`
- Result: repository gates passed; 83 Python tests passed; Rust line coverage
  80.11%; audit, deny, secret scan, release-size, shell and packaging gates passed.

## Guarantees

| Guarantee | Evidence |
|---|---|
| The standalone WLOC package is named `wificalling-location-gateway`, matching the project instead of looking like a LuCI-only component | Filename and control package-name assertions |
| The output identifies the AX6S runtime as `aarch64_cortex-a53`, not `all` | Package filename and control metadata assertions |
| The package has no dependency on separate Gateway or WLOC packages | Exact `Depends` and negative dependency assertions |
| The package has no Gateway IPK input or Gateway compatibility metadata | Builder contract and negative dependency assertions |
| WLOC UCI and CA survive reinstall/upgrade | Exact `conffiles` and migration assertions |
| WLOC init/config, service, control client, provider detector, and UI are present | Required payload member assertions |
| A mismatched device architecture or firmware family stops preflight | Negative compatibility tests |

## Scope and gap

The current V2 package target is
`wificalling-location-gateway_2.0.0-1_aarch64_cortex-a53.ipk`. The final
architecture-correct AArch64 cross-build is 2,035,872 bytes for
`wloc-service` and 462,792 bytes for `wloc-ctl`; both are static ELF artifacts.
The release package SHA-256 is emitted in `SHA256SUMS`. A detached release
signature still requires the protected release signing key and is intentionally
not committed.

This document records package-construction evidence and links to the separate
redacted AX6S installation/resource/rollback evidence. The real device was
tested after removing the old WLOC package, while preserving the selected
tiny/lite/PassWall provider. The release candidate package was built and
verified after that run; device-update evidence is recorded by package family
and exact tested versions rather than being conflated with the later release
filename.
