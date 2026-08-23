# Agent handoff: Issue 66

## Identity and scope

- Source agent ID: codex-release
- Capabilities used: openwrt,test,ci,integration
- Branch: codex/issue-66-standard-lite-variants-codex-release-20260823041333-31bfd3b9
- Stable base commit: `81d5f6b`
- Updated at (UTC): 2026-08-23
- Credentials included: no

## Objective

Publish Standard and Lite installation variants of the same integrated 1.2.x
product for all three existing package targets, while preserving the AX6S
low-memory/low-storage behavior and both UCI configurations.

## Completed

- Added mutually exclusive Standard and Lite package metadata and six-asset
  release planning. Standard uses the firmware sing-box; Lite explicitly
  provides/replaces the sing-box package contract.
- Added a hash-pinned Lite runtime packager. It keeps a compressed payload on
  flash and installs a transparent wrapper that prepares one verified copy in
  tmpfs for WCG and PassWall.
- Applied `GOMAXPROCS=1`, `GOMEMLIMIT=24MiB`, and `GOGC=75` only to the Lite WCG
  process; Standard inherits firmware defaults.
- Expanded the pinned Docker matrix to eight Standard/Lite runtime rows and
  verified executable ownership as well as install/start/socket/status.
- Built all six r5 assets, installed the exact corrected AArch64 Lite asset on
  AX6S, preserved both UCI files, cold-booted, and observed a real iPhone WLOC
  request being synthesized.
- Updated English/Chinese README, English/Chinese changelog notes, licensing,
  checksums, and release evidence.

## Verification

| Gate | Result |
|---|---|
| Packaging TDD | Passed |
| Six package builds | Passed |
| Eight-row Docker matrix | All installed, started, socket-ok, status-ok |
| `./scripts/ci/verify.sh` | Passed; 69 Python tests, all Rust/JS suites, 81.40% line coverage, audits, licenses, secrets and repository gates |
| AX6S Lite migration | UCI hashes unchanged; overlay ~20.4 MB free |
| AX6S cold boot | tmpfs runtime hash, WCG/WLOC, nftables and status passed |
| iPhone WLOC | Assigned device request observed and synthesized |

Exact package hashes and hardware evidence are in
`docs/testing/V1.2.2_R5_STANDARD_LITE_RELEASE.tdd.md`. The release directory
also contains the upstream sing-box v1.12.25 source archive and its independent
source checksum manifest for GPL compliance.

## Failed attempts

- The first dual-package SDK build placed conflicting package definitions in
  one SDK config and hit a Kconfig dependency cycle. Standard and Lite now use
  isolated SDK containers.
- The first AX6S Lite candidate stored the 29.7 MB ELF directly on overlay and
  left 2.8 MB free. It was rejected, replaced by compressed flash storage plus
  tmpfs preparation, and never published.
- Minimal rootfs opkg loses temporary architecture declarations after install;
  ownership checks now read opkg's persisted file list.
- OpenWrt 25 cannot fetch packages after its firewall starts under Docker
  Desktop. APK dependencies are resolved before `init`, then runtime checks run
  after boot.

## Next executable steps

1. Push the branch and open a stacked PR against Issue #64's stable branch.
2. Wait for GitHub CI and obtain independent review.
3. Publish tag `v1.2.2-r5` with six packages, `SHA256SUMS`, Docker report,
   sing-box v1.12.25 source archive and `SOURCE_SHA256SUMS`.
4. Re-download every release asset and verify both checksum manifests.

## Capabilities required for the next Agent

- openwrt
- test
- ci
- release
- security-review

## Security and privacy notes

- No credentials, node secrets, CA private keys, precise locations, or raw
  device payloads are included.
- Router evidence records only the existing test IP and public service host;
  proxy credentials and exact saved UCI contents are excluded.
- Interception scope, UDP 500/4500 behavior, WCG/WLOC schemas and Beta project
  separation were not broadened.
