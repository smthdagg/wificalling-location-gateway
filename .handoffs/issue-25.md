# Agent handoff: Issue 25

## Identity and scope

- Source agent ID: codex-release-1-0
- Capabilities used: rust,openwrt,ci,test,docs,release
- Branch: codex/issue-25-release-1-0
- Checkpoint parent: c7818330cab7c2ebfcd3c37f474258d760ea394e
- Updated at (UTC): 2026-08-13T11:02:39Z
- Credentials included: no

## Objective

Ship the standalone Wi-Fi Calling Location Gateway 1.0 release as one project-named package per supported platform, migrate the authorized AX6S without clearing configuration, and publish verified release assets.

## Completed

- Froze version 1.0.0 across Cargo, OpenWrt metadata, builders, tests, and documentation.
- Built a complete AX6S AArch64 IPK and integrated x86-64 IPK/APK assets.
- Preserved both Gateway and WLOC UCI paths as package conffiles.
- Passed pinned AArch64 OpenWrt 24.10.5, x86-64 OpenWrt 24.10.8, iStoreOS 24.10.5, and OpenWrt 25.12.3 Docker install/start/socket/status checks, covering every release asset.
- Updated README, deployment instructions, packaging evidence, changelog, and release notes.

## Verification

| Command | Result | Evidence |
|---|---|---|
| `./scripts/ci/verify.sh` | Passed | 67 Python tests, complete Rust suite, 80.32% line coverage, audit/deny and secret scan |
| `./scripts/openwrt/verify-docker-matrix.sh --dist-dir "$PWD/dist/v1.0.0"` | Passed | All three release assets across four environments installed, started, created the socket, and returned v1 status |
| AX6S package inspection | Passed | Project package name, AArch64 architecture, complete payload, system-only dependencies, and two conffiles |
| Release SHA-256 generation | Passed | Three architecture/package-manager assets recorded in `SHA256SUMS` |

## Failed attempts

- The first 25.x matrix run used a fake dependency-provider APK that conflicted with real rootfs packages; the test now forbids that provider and uses the actual rootfs facilities.
- The first generated post-install warning let Make consume the shell variable prefix; a regression test now requires the escaped variable and the final package prints complete prerequisite paths.

## Next executable steps

1. Log into the authorized AX6S through the local LuCI session.
2. Back up the router and hash both UCI conffiles without exposing their contents.
3. Install the exact 1.0.0 AArch64 package directly and verify configuration retention, services, socket/status, LuCI, and Auto-to-Manual switching.
4. Obtain independent review, mark the PR ready, merge after CI, then tag and publish v1.0.0 with the three packages and `SHA256SUMS`.

## Capabilities required for the next Agent

- openwrt
- ax6s
- ci
- security
- release

## Security and privacy notes

- No credentials, node links, CA private keys, device identifiers, raw WLOC payloads, or precise personal locations are included.
- The release preserves WLOC isolation from UDP 500/4500 and the Gateway nftables table.
- The AX6S upgrade must not remove packages first or print configuration contents.
