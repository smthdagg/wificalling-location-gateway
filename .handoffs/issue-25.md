# Agent handoff: Issue 25

## Identity and scope

- Source agent ID: codex-release-1-0
- Capabilities used: rust,openwrt,ci,test,docs,release
- Branch: codex/issue-25-release-1-0
- Checkpoint parent: 4fafb327f59642a92fe7e2f5e9b5f6454266bf2c
- Updated at (UTC): 2026-08-13T13:18:37Z
- Credentials included: no

## Objective

Ship the standalone Wi-Fi Calling Location Gateway 1.0 release as one project-named package per supported platform, migrate the authorized AX6S without clearing configuration, and publish verified release assets.

## Completed

- Froze version 1.0.0 across Cargo, OpenWrt metadata, builders, tests, and documentation.
- Built a complete AX6S AArch64 IPK and integrated x86-64 IPK/APK assets.
- Preserved both Gateway and WLOC UCI paths as package conffiles.
- Passed pinned AArch64 OpenWrt 24.10.5, x86-64 OpenWrt 24.10.8, iStoreOS 24.10.5, and OpenWrt 25.12.3 Docker install/start/socket/status checks, covering every release asset.
- Updated README, deployment instructions, packaging evidence, changelog, and release notes.
- Installed the exact final AX6S asset; both UCI hashes, services, restored Wi-Fi Calling settings, and LuCI Manual → Auto → Manual passed.
- Fixed the clean-install runtime-directory and suffixed LuCI-cache defects found during real-device validation.
- Removed the duplicate standalone Gateway LuCI menu from the integrated package; AX6S now exposes one package and one `WifiCalling&Wloc Gateway` menu only.
- Fixed WLOC auto/manual switching: the RPC bridge no longer re-enables an already-intercepting daemon and therefore no longer fails with `invalid_config`.
- Made fixed-coordinate manual switching local-only so an optional online reverse-geocode lookup cannot block the root-only control socket.
- Fixed FAQ tab localization by reapplying the selected language after LuCI finishes replacing the tab bar.
- Rebuilt and installed the updated AArch64 package on AX6S. Manual and Auto each passed switch -> Save & Apply -> Monitor & Log verification; FAQ retained all five Chinese labels. Router was restored to Auto after testing.
- Confirmed the AX6S still has only `wificalling-location-gateway - 1.0.0-1`, one integrated menu file, and the Wi-Fi Calling configuration hash remained unchanged.

## Verification

| Command | Result | Evidence |
|---|---|---|
| `./scripts/ci/verify.sh` before latest mode fix | Passed | 69 Python tests, complete Rust suite, audit/deny and secret scan |
| Latest targeted mode/i18n/Rust tests | Passed | LuCI mode, deferred tab localization, local-only manual switch, HTTP reverse-geocode positive/negative tests |
| Latest full coverage gate | **Pending/fails threshold only** | All 46 library tests and all integration tests pass; total line coverage is 79.81%, below the required 80% by 0.19 percentage points |
| `./scripts/openwrt/verify-docker-matrix.sh --dist-dir "$PWD/dist/v1.0.0"` | Passed | All three release assets across four environments installed, started, created the socket, and returned v1 status |
| AX6S package inspection | Passed | Project package name, AArch64 architecture, complete payload, system-only dependencies, and two conffiles |
| Release SHA-256 generation | Passed | Three architecture/package-manager assets recorded in `SHA256SUMS` |

## Failed attempts

- The first 25.x matrix run used a fake dependency-provider APK that conflicted with real rootfs packages; the test now forbids that provider and uses the actual rootfs facilities.
- The first generated post-install warning let Make consume the shell variable prefix; a regression test now requires the escaped variable and the final package prints complete prerequisite paths.
- The first mode fix removed the duplicate enable call, but real AX6S manual mode still blocked on synchronous reverse geocoding. The final implementation removes external network I/O from the coordinate control path.
- Removing synchronous reverse geocoding reduced exercised lines. Two offline HTTP tests restored most coverage, but the current aggregate is 79.81%; do not mark CI green until at least 0.19 percentage points of meaningful coverage are added.

## Next executable steps

1. Add a small meaningful offline test to lift aggregate Rust line coverage from 79.81% to at least 80%, then rerun `./scripts/ci/verify.sh`.
2. Re-run the Docker package matrix because the AArch64 runtime changed after the previously recorded matrix.
3. Rebuild all release assets and `SHA256SUMS`; the last AX6S-only package installed during validation had SHA-256 `fcae15e88234ac2f000e5d25db36f714d90704a16d6f345d5e224d264fa7fd02`.
4. Obtain independent re-review of the menu, mode-switch, localization, and release-integrity fixes.
5. Mark PR #27 ready, merge after GitHub CI, then tag and publish v1.0.0. Do not release the older assets.

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
