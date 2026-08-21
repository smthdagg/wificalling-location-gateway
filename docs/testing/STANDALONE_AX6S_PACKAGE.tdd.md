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
`wificalling-location-gateway_2.0.0-1_aarch64_cortex-a53.ipk`. The latest local
host release binaries are 1,654,336 bytes (`wloc-service`) and 369,488 bytes
(`wloc-ctl`); they are not evidence of an AArch64 ELF cross-build. A final
package SHA-256 and signed release manifest still require the
architecture-correct release build.

This document records package-construction evidence only. It does not claim
that the final standalone package has been installed on AX6S; that claim
requires the documented removal of the old WLOC package, preservation of the
selected tiny/lite/PassWall provider, and redacted migration/resource/rollback
evidence.
