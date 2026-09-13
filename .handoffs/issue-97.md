# Agent handoff: Issue 97

## Identity and scope

- Source agent ID: codex-wfc-ipv6-20260912235843
- Capabilities used: shell, OpenWrt, CI, release
- Branch: `codex/issue-97-wfc-ipv6-passwall-codex-wfc-ipv6-20260912235843-2d8648ef`
- Checkpoint parent: `1ee02cb`
- Updated at (UTC): 2026-09-13
- Credentials included: no

## Objective

Fix WFC and WLOC IPv4/IPv6 coexistence with PassWall, including DNS/CDN
handling, service route isolation, cleanup, and release packaging.

## Completed

- WFC IPv6 is fail-open from the IPv4 PassWall bypass and TPROXY path.
- WLOC collects AAAA CDN targets, uses an earlier mangle priority, rejects
  Apple IPv6 targets from the IPv4 MITM path, and removes DNS service sections
  on stop.
- Added regression coverage for the runtime contracts and fixed the release
  matrix's minimal-rootfs `/usr/sbin/ip` compatibility fixture.
- Prepared `v1.3.0-r14` standard/Lite packages for aarch64, x86_64 IPK and
  x86_64 APK; full CI verification passed.
- Published signed WLOC feed indexes and the GitHub release.

## Verification

- `./scripts/ci/verify.sh`: passed; 69 Python tests, Rust suites, JS tests,
  packaging/version checks, secret scan, audit, and coverage gates.
- Six package SHA256 checks: passed.
- Feed `scripts/feed-verify.sh`: passed.
- Live AX6S combined WFC/WLOC service and route cleanup verification: passed.

## Next executable steps

- Review and merge PR #98.

## Security and privacy notes

- No credentials are included. The feed signing private key remains local and
  was not committed.
