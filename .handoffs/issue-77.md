# Agent handoff: Issue 77

## Identity and scope

- Source agent ID: codex-release-r9
- Capabilities used: rust,openwrt,security,test,ci,docs
- Branch: codex/issue-77-wloc-r9-release-codex_release_r9_20260829-20260828161811-50e26299
- Checkpoint parent: `ecfb5fe` (v1.3.0-r8)
- Updated at (UTC): 2026-08-29
- Credentials included: no

## Objective

Publish v1.3.0-r9 for the stable integrated 1.3.0-r1 baseline. The release
fixes WLOC fail-open startup ordering, prevents stale AX6S tmpfs
runtime/cache accumulation during upgrades, and changes node health to
bounded direct ICMP endpoint checks so it never launches a second sing-box.

## Completed

- Intercept exactly six Apple WLOC hostnames (`gs-loc.apple.com`,
  `gs-loc-cn.apple.com`, `gsp-ssl.ls.apple.com`,
  `bluedot.is.autonavi.com`, `bluedot.is.autonavi.com.gds.alibabadns.com`,
  `gspe19-cn-ssl-ls-apple-com.v.aaplimg.com`).
- Forward Apple's response and minimally replace only existing latitude,
  longitude, and horizontal accuracy; all other Location fields, root
  records, and unknown fields pass through byte-for-byte.
- Handle HTTP/2 streams independently (per-stream tasks, 30 s request-body
  bound) so a slow speculative POST cannot block a location response.
- Treat rejected unrelated client TLS/SNI as an isolation-boundary success,
  not a WLOC failure; each daemon start writes a fresh proxy-health
  snapshot.
- WLOC installs TPROXY only after Gateway/location health is verified;
  unavailable states remain fail-open and recover automatically (verified
  live on AX6S: sing-box kill -9 → procd respawn <2 s → WLOC re-intercepts).
- Node health uses one bounded ICMP endpoint probe at a time (mkdir lock +
  round-robin cursor + 60 s cache), has no curl/loopback proxy path and no
  temporary sing-box; LuCI renders reachable/unreachable/unknown.
- Lite runtime profile: `GOGC=50` + `GOMEMLIMIT=48MiB` + startup
  available-memory preflight (32 MiB, 64 MiB during cold Lite tmpfs
  expansion).
- Upgrade preinst removes only its exact stopped `/tmp/sing-box-lite*` and
  `/tmp/node-health-*` files; no blanket /tmp deletion (regression tests
  added).
- Lite sing-box runtime is the hash-pinned binary from the published
  v1.3.0-r8 asset (aarch64 `7f98d917…`, x86_64 `d14cf69a…`); verified
  byte-for-byte inside the new packages.
- Bumped package release metadata to `1.3.0-r9`, bilingual README,
  changelog, protocol/threat-model docs, fixture schema, security
  invariants, and release tests.
- Fixed the Docker matrix smoke environment for OpenWrt 25.12 (minimal
  rootfs lacks `/etc/config/network`; provision a LAN section) and the
  pinned-image probe in the Rust cross-build scripts (accept tag- or
  digest-only image presence).
- Built all six Standard/Lite assets for AX6S AArch64, OpenWrt 24.10
  x86_64 (IPK) and OpenWrt 25.12 x86_64 (native APK).

## Verification

- `./scripts/ci/verify.sh` full gate: Rust tests + 78.14% line coverage,
  69 Python tests, JS regression, packaging/version tests, secret scan,
  cargo audit/deny all pass.
- OpenWrt Docker install matrix 8/8: AX6S 24.10.5, OpenWrt 24.10.8,
  OpenWrt 25.12.3 (APK), iStoreOS 24.10.5 × Standard/Lite —
  installed/started/socket-ok/status-ok.
- Live AX6S (192.168.31.1) cleaned upgrade to 1.3.0-r9 Lite:
  `/etc/config/{wloc-service,wificalling-gateway}` byte-identical before
  and after; single shared sing-box; `GOMAXPROCS=1 GOGC=50
  GOMEMLIMIT=48MiB`; `MemAvailable` above the 32 MiB gate; WLOC
  `intercepting`; proxy-health snapshot fresh; node-status JSON generated.
- Feed repository gh-pages index regenerated and re-signed with the
  long-lived key; AX6S `opkg update` printed `Signature check passed` for
  the wloc feed and listed the r9 packages.

## Failed attempts

- Docker install matrix initially failed on the OpenWrt 25.12 APK case
  (minimal rootfs has no `/etc/config/network`; `uci` calls aborted the
  matrix script under `set -e`, and `wloc-redirect-sync prepare` refused to
  start without a LAN IPv4). Fixed by provisioning a minimal LAN section
  file in the smoke container only.
- A first LAN-config fix using `uci set`/`uci commit` itself aborted the
  matrix (the `network` package does not exist in that rootfs); replaced
  with a direct `/etc/config/network` file write.
- The PR `openwrt-cross-build` check failed because the workflow pulls the
  pinned Rust image by digest without aliasing the tag, while the build
  scripts probe the tag. Fixed by tagging after the pull and by accepting
  either tag or digest in both scripts.
- The PR `pull-request-contract` check failed twice: the handoff capsule
  was missing, then its headings did not match the contract. Both fixed in
  this branch.

## Next executable steps

- Merge this release PR after CI is green.
- Tag `v1.3.0-r9` at the merged commit and upload the six packages,
  `SHA256SUMS`, signed `Packages`/`Packages.gz` (+ `.sig`) and bilingual
  release notes.
- Confirm the feed upgrade path on AX6S (already validated: `opkg update`
  signature passes with key `f7050198aa77cf15`; installed version now
  `1.3.0-r9`).
- Close issue #77 once the release is published.

## Capabilities required for the next Agent

- GitHub CLI (`gh`) with write access to `smthdagg/wificalling-location-gateway`
  and `smthdagg/wificalling-location-gateway-feed`.
- Docker for feed signing (`ghcr.io/openwrt/rootfs:x86_64-24.10.8` pinned
  image) and for the install matrix.
- SSH access to the AX6S test router (`192.168.31.1`, root) for upgrade
  validation.

## Security and privacy notes

- No credentials are included in this capsule or in the repository.
- The feed signing private key lives only at `~/.zcode/keys/wloc-signing.key`
  (mode 0600) on the release machine and is never committed.
- No raw WLOC request/response bodies, device IPs, or location coordinates
  were recorded; logs and health files contain only counts and status
  fields.
- The raw-WLOC dump paths (`WLOC_DUMP_DIR`, `/tmp/wloc-forward.dump`) were
  removed in earlier releases; the r9 audit confirmed no such paths remain.