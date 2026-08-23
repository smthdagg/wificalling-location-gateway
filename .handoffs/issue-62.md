# Agent handoff: Issue 62

## Identity and scope

- Source agent ID: codex-v122-r3-release
- Capabilities used: rust,openwrt,ax6s,ci,test,docs,release
- Branch: codex/issue-62-v1.2.2-r3-platform-release
- Stable baseline: tag `v1.2.2-r3` (`123e22ed573c5359070c38f039a422dc5137f404`)
- Updated at (UTC): 2026-08-23
- Credentials included: no

## Objective

Complete the stable `v1.2.2-r3` release with one integrated package for each
supported platform, verify the packages, and update bilingual installation and
English bug-fix documentation without importing multi-device Beta code.

## Completed

- Added a RED/GREEN regression test and fixed the release builder to accept a
  SHA-256-pinned stable integrated 1.2.x base package.
- Built the OpenWrt/iStoreOS 24.x x86_64 IPK and OpenWrt 25.12 x86_64 native
  APK from the stable source with pinned SDK/toolchain images.
- Retained the already-published and AX6S-verified AArch64 IPK byte-for-byte.
- Passed all three release assets through the four-environment Docker matrix.
- Re-verified package/service health and PID stability on the Redmi AX6S.
- Updated the English and Chinese README sections, changelog, packaging guide,
  and release-specific TDD/verification evidence.
- Uploaded the two x86_64 packages, SHA256SUMS, and Docker matrix report to the
  existing GitHub Release. The existing tag and AArch64 asset were not moved or
  replaced.

## Verification

| Gate | Result |
|---|---|
| `./tests/scripts/test-openwrt-release-packaging.sh` | Passed after expected RED checkpoint |
| `./scripts/ci/verify.sh` | Passed; 69 Python tests, all Rust suites, 81.06% line coverage, audit/license/secret/repository gates |
| Four-environment Docker matrix | All installed, started, socket-ok, status-ok |
| AX6S runtime | `1.2.2-r3`; WCG, sing-box, WLOC, config and socket healthy; PIDs stable |
| GitHub re-download and SHA check | All three package assets OK |

## Release hashes

```text
b41641f86ba4ced4f0278167d5627b17da5c1c9fdcb7bdf67f679cf80d75c8d8  wificalling-location-gateway_1.2.2-r3_aarch64_cortex-a53.ipk
5de6d6a68e3b78d2c09fa2a1ebab65be5913686785ef91083368f3f143c03944  wificalling-location-gateway_1.2.2-r3_x86_64.ipk
e43e12fd0b87a34b517472903d16fb251cdc17be8de6a2728eab3546d61ff95c  wificalling-location-gateway-1.2.2-r3.apk
```

## Review focus

- Confirm the package-identity/version allowlist remains narrow and still
  requires the explicit SHA-256 pin.
- Confirm README evidence does not claim fresh carrier or iPhone traffic that
  was not initiated during this release pass.
- Confirm no Beta/multi-device files entered the stable branch.

## Security and privacy notes

- No credentials, node secrets, CA private keys, device identifiers, raw WLOC
  traffic, or precise location values are included.
- The release does not broaden interception scope or alter UDP 500/4500 rules.
- Low-storage AX6S guidance requires backing up and preserving both UCI files
  when an older package must be removed before installation.
