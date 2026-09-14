# Agent handoff: Issue 105

## Identity and scope

- Source agent ID: zcode-wloc-route-lifecycle-20260914
- Capabilities used: openwrt, ci, release
- Branch: codex/issue-105-route-lifecycle-zcode-wloc-route-lifecycle-20260914
- Checkpoint parent: `3b4343a`
- Updated at (UTC): 2026-09-14
- Credentials included: no

## Objective

Withdraw the static-route/interception state (ip rule, ip route, nftables
table, DNS hijack) whenever the device scope becomes empty: deleting the
last bound device or stopping a service must remove the stale state, and
enabling the service + binding a device must (re)install it. Empty-scope
syncs must be fail-open instead of exiting while leaving prior state behind.

## Completed

- `wloc-redirect-sync.sh` (both `openwrt/files/usr/sbin/` and
  `scripts/openwrt/` copies): the no-device branch now invokes its own stop
  path (`"$0" stop`) before exiting 1, withdrawing rule/route/nft/DNS state
  installed for a previously bound device.
- `firewall.sh`: the empty-client-set branch now clears the PassWall bypass,
  deletes the `wificalling_gateway` nft table, removes the `0x66/table 166`
  rule, and flushes `table 166` before exiting 0 — identical to the stop
  path.
- `tests/scripts/test-wloc-runtime-contract.sh`: new assertions pin the
  route lifecycle (add rule+route on install, delete on stop, self-withdraw
  on empty scope, gateway flush on stop and empty client set).
- Version bump r15 -&gt; r16 (Makefiles, builder scripts, version/package
  tests, README, CHANGELOG).
- Docker stub simulation verified all four lifecycle directions:
  empty-scope start withdraws, start-with-device adds rule+route+nft,
  explicit stop removes, gateway empty-clients flushes table 166.

## Verification

- `./scripts/ci/verify.sh`: passed (exit 0) — Rust tests, Python tests, JS
  tests, packaging/version tests, secret scan, audit, coverage.
- `test-wloc-runtime-contract.sh`: passed with the new lifecycle assertions.
- Behavior simulation in `alpine` Docker with stub `uci`/`ip`/`nft`: all
  three redirect-sync directions matched the contract.
- `koalaman/shellcheck:stable` on the three changed scripts: only pre-existing
  info-level style notes; no new findings.

## Failed attempts

- The first stub-based simulation used a non-POSIX `${@: -1}` sed stub that
  errored under busybox; the stubs were rewritten POSIX-safe. No repository
  code was affected.

## Next executable steps

- Merge PR #105 once CI is green, then cut release v1.3.0-r16 per
  `docs/releases/RELEASE_PROCESS.md` (feed update before tagging, signed
  feed, GitHub release, AX6S upgrade + signature check and post-upgrade
  hygiene).

## Capabilities required for the next Agent

- GitHub CLI with access to wificalling-location-gateway and the
  Smthdagg-Repo-feeds repository.
- Docker for OpenWrt package, matrix, and feed verification.
- Local access to the feed signing key (`~/.zcode/keys/wloc-signing.key`).

## Security and privacy notes

- No credentials are included. The feed signing private key remains local and
  was not committed.
