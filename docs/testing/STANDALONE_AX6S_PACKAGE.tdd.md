# Standalone AX6S package — TDD evidence

## User journey

As an AX6S administrator, I can install one architecture-specific IPK that restores both Wi-Fi Calling Gateway and WLOC without clearing either existing UCI configuration.

## RED

- Command: `./tests/scripts/test-standalone-ax6s-package.sh`
- Result before implementation: `unsupported dependency mode: ax6s-standalone`
- A second boundary test then failed because the first implementation incorrectly retained an `all` architecture filename and metadata while carrying AArch64 executables.
- The naming refinement produced RED with `FAIL: standalone package filename must use the project name and identify the AX6S architecture` before the complete product package was renamed.

## GREEN

- Command: `./tests/scripts/test-standalone-ax6s-package.sh`
- Result: `standalone AX6S package tests passed`
- Full command: `./scripts/ci/verify.sh`
- Result: repository gates passed; 67 Python tests passed; Rust line coverage 80.32%; audit, deny, secret scan, release-size, shell and packaging gates passed.

## Guarantees

| Guarantee | Evidence |
|---|---|
| The complete product package is named `wificalling-location-gateway`, matching the project instead of looking like a LuCI-only component | Filename and control package-name assertions |
| The output identifies the AX6S runtime as `aarch64_cortex-a53`, not `all` | Package filename and control metadata assertions |
| The integrated package has no dependency on separate Gateway or WLOC packages | Exact `Depends` and negative dependency assertions |
| Gateway 1.7.x input has the expected identity and pinned SHA-256 | Identity/version and digest rejection tests |
| Both UCI files survive reinstall/upgrade | Exact `conffiles` assertions for both paths |
| Gateway init/config, WLOC init/config, service and control client are all present | Required payload member assertions |
| A mismatched Gateway package digest stops the build | Negative SHA-256 test |

## Scope and gap

The formal release artifact is
`wificalling-location-gateway_1.0.0-1_aarch64_cortex-a53.ipk`. Its product
binaries were rebuilt from the version 1.0.0 source with the pinned mt7622
toolchain: `wloc-service` is a static AArch64 ELF of 1,904,800 bytes and
`wloc-ctl` is a static AArch64 ELF of 462,792 bytes. The package SHA-256 is
`7565a77ae36917ce1898134b6f1a7e7c7b50790335f1a39ff2f89745148f8f0f`
and is also recorded in the release `SHA256SUMS`.

The exact final asset was installed over the authorized ImmortalWrt 24.10.6
AX6S. Both UCI hashes remained unchanged, both services ran, the restored
Wi-Fi Calling settings rendered existing policies, and LuCI Manual → Auto →
Manual completed without a socket error. “Standalone” means no separate
Gateway or WLOC application package; normal OpenWrt facilities remain required.
