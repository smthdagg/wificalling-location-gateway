# Agent handoff: Issue 79

## Identity and scope

- Source agent ID: zcode-audit-r10-20260829
- Capabilities used: rust,openwrt,security,test,ci,docs
- Branch: codex/issue-79-r10-audit-fixes
- Checkpoint parent: `c6492df` (v1.3.0-r9)
- Updated at (UTC): 2026-08-29
- Credentials included: no

## Objective

Publish v1.3.0-r10 for the stable integrated 1.3.0-r1 baseline, closing the
P0/P1 findings from the full-code audit at `c6492df` while preserving the r9
fail-open architecture.

## Completed

- P0: every TPROXY install refreshes the upstream map — the redirect install
  path calls `wloc-refresh-set.sh` and fails closed when DNS is unavailable,
  so disable/enable and sing-box crash recovery can no longer leave
  interception installed without it.
- P1: the init gates interception-side state on `wloc-service.main.enabled`;
  disabled starts withdraw the DNS hijack, TPROXY, and upstream map while the
  daemon still runs (control API stays reachable).
- P1: the stop path no longer resets `dns_changed` after removing the DNS
  hijack block; the running dnsmasq is restarted as intended.
- P1: `enable()` records the desired state before any fallible step, so a
  failed startup enable is retried by the periodic tick.
- P1: UCI `probe_interval` is clamped to the probe validator's window
  (30..300 s, ceiling imported from `exitprobe::MAX_OBSERVATION_AGE`).
- P1: the exit probe uses a bounded native HTTP client (absolute-form URI
  against the Gateway loopback inbound); the external `curl` dependency is
  gone, so auto-mode enables on clean OpenWrt images.
- LuCI: shipped `overview.js` recognizes the ICMP `reachable` nodeTest state;
  `wloc-monitor.js` renames the Clear-log button so the rpc function is no
  longer shadowed; `wloc-health.sh` captures the inner `last_error` string and
  re-wraps it (no more invalid JSON during probe failures); the FAQ describes
  the ICMP reality; `/proc/net/arp` is granted to the gateway ACL.
- Tooling: `verify.sh` warns when the ShellCheck gate is skipped; the
  `scripts/openwrt/wloc-redirect-sync.sh` dev mirror is byte-identical to the
  shipped copy; package release metadata bumped to `1.3.0-r10` with bilingual
  README, changelog, and release-test updates.

## Verification

- `cargo fmt --check`, `cargo clippy --locked --all-targets -D warnings`, and
  the full `cargo test` suite pass (including new probe-mock and clamp tests).
- `./scripts/ci/verify.sh` full gate passes: Python 69, JS, packaging, secret
  scan, cargo audit/deny, and the updated runtime-contract test that pins the
  new invariants (map-before-tproxy, enabled gate, single `dns_changed=0` in
  the stop block).
- Docker ShellCheck (v0.10.0, CI-equivalent invocation): zero findings.

## Failed attempts

- The first contract assertion for the `dns_changed` fix counted occurrences
  in the whole file and failed because the start path legitimately contains
  its own `dns_changed=0`; rewritten to extract the stop block and count
  inside it.
- The init-ordering assertion used first-match awk line numbers and broke when
  the disabled branch started the daemon earlier than the enabled branch's
  refresh-set; rewritten to compare the last occurrences.

## Next executable steps

- Merge the release PR after CI is green.
- Rebuild both runtimes (Rust sources changed) and the six Standard/Lite
  assets, run the Docker install matrix, update and re-sign the feed.
- Tag `v1.3.0-r10` at the merged commit, publish the release, and run the
  live AX6S r9-to-r10 upgrade including the `opkg update` signature check.

## Capabilities required for the next Agent

- GitHub CLI (`gh`) with write access to `smthdagg/wificalling-location-gateway`
  and `smthdagg/wificalling-location-gateway-feed`.
- Docker for the pinned cross builds, feed signing, and the install matrix.
- SSH access to the AX6S test router (`192.168.31.1`, root) for the live
  upgrade validation.

## Security and privacy notes

- No credentials are included in this capsule or in the repository.
- The feed signing private key lives only at `~/.zcode/keys/wloc-signing.key`
  (mode 0600) on the release machine and is never committed.
- The native probe client sends one plain-HTTP request to ip-api.com through
  the Gateway's loopback inbound, exactly like the previous curl path; no raw
  WLOC traffic, device IPs, or location coordinates are recorded anywhere.
