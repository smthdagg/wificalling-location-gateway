# Agent handoff: Issue 89

## Identity and scope

- Source agent ID: zcode-memgate-r13-20260829
- Capabilities used: openwrt,test,ci,docs
- Branch: codex/issue-89-r13-memory-gate
- Checkpoint parent: `6bc3ec9` (v1.3.0-r12)
- Updated at (UTC): 2026-08-29
- Credentials included: no

## Objective

Publish v1.3.0-r13 replacing the flat 32/64 MiB start-time memory thresholds
with a computed requirement plus a bounded self-healing retry, and removing
the redundant success popup after a log clear.

## Completed

- `wificalling-gateway` init: `require_start_memory` now computes the real
  requirement — Lite cold start streams the compressed runtime through
  `wc -c` for the exact inflated size plus an 8 MiB margin; warm and standard
  starts reserve 8 MiB only (the runtime heap is bounded by GOMEMLIMIT).
- A refused start schedules `schedule_memory_retry`: a locked background loop
  (30 s x 20, 10-minute window) that retries the service start and logs the
  outcome, so a temporarily memory-tight router self-heals instead of staying
  down until a manual restart.
- `wloc-monitor.js` / `wfc-monitor.js`: clearing a log now closes the confirm
  modal and empties the list without a second popup; error notifications are
  kept.
- Package release metadata bumped to `1.3.0-r13` with bilingual README,
  changelog, and release-test updates (the variant assertions now pin the
  computed-gate markers and the retry).

## Verification

- `sh -n` on the init; all seven JS suites pass; full `./scripts/ci/verify.sh`
  gate green with the updated assertions.
- Live evidence for the design: the v1.3.0-r10 upgrade refused the gateway
  with `insufficient available memory (52688 KiB; need 65536 KiB)` while an
  upgrade ipk sat in /tmp, and stayed down until tmpfs was manually freed —
  exactly the scenario the retry now self-heals.

## Failed attempts

- The first `gh issue create` for this capsule failed wholesale because the
  label `role:openwrt` does not exist in the repository; recreated with valid
  labels.

## Next executable steps

- Merge the release PR after CI is green.
- Build the six Standard/Lite assets (Rust runtimes reused — no Rust change),
  run the Docker install matrix, update and re-sign the feed.
- Tag `v1.3.0-r13`, publish the release, and run the live AX6S r12-to-r13
  upgrade: the cold-start gate must now pass under the old failure
  conditions.

## Capabilities required for the next Agent

- GitHub CLI (`gh`) with write access to `smthdagg/wificalling-location-gateway`
  and `smthdagg/wificalling-location-gateway-feed`.
- Docker for feed signing and the install matrix.
- SSH access to the AX6S test router (`192.168.31.1`, root) for the live
  upgrade validation.

## Security and privacy notes

- No credentials are included in this capsule or in the repository.
- The gate change only alters when the router starts its own services; no
  data-handling or exposure change.
