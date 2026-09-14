# Agent handoff: Issue 107

## Identity and scope

- Source agent ID: zcode-audit-hardening-20260915
- Capabilities used: openwrt, go-free-rust, luci, ci, release
- Branch: codex/issue-107-audit-hardening-zcode-audit-hardening-20260915
- Checkpoint parent: `413f9df`
- Updated at (UTC): 2026-09-15
- Credentials included: no

## Objective

Comprehensive post-r16 audit across four slices (shell lifecycle, Rust
daemon, LuCI/rpcd, build/docs/tests) and repair of every P1 finding plus the
actionable P2 hardening, before cutting the v1.3.0-r16 release.

## Completed

- Control plane: dispatch and housekeeping moved to a dedicated worker
  thread owning the handler (bounded job queue, per-connection tasks under a
  semaphore), so bounded blocking probe/Geo I/O can no longer wedge the
  accept loop or starve status publication. Runtime traits gained Send.
- Fail-closed config: corrupt UCI now falls back to enabled=false; startup
  reconciles a disabled config by withdrawing any stale redirect.
- MITM hardening: chunked decoder rejects oversized chunk sizes and uses
  checked arithmetic (no slicing panic); upstream selection appends the
  refreshed per-host map candidates as fallbacks; marker scan capped at 64
  candidates; status/health files written atomically; event appends
  serialized; CA key created 0600 and repaired on load; probe inbound ports
  colliding with the MITM port are skipped; geo logs no longer print
  coordinates; wloc-ctl gained 15 s timeouts and a response cap.
- Shell lifecycle: wloc-redirect-sync.sh arms a recursion-guarded fail-open
  EXIT trap that withdraws all owned state on any nonzero exit; the stop
  branch guards dnsmasq/passwall activation steps; refresh-set only flags
  DNS changes when the domain list actually differs and suffixes the
  upstream-map tmp with $$; firewall.sh guards the bypass clear; node-health
  treats missing/garbage lock pids as stale; the memory-retry loop aborts
  when the service was disabled mid-window; wloc-health.sh prefers stat over
  busybox date -r; monitor.sh exits on TERM.
- LuCI: enable toggle reverts on daemon-level errors; mode-select resets on
  failure; applyPreset applies through the daemon before persisting; the
  add-preset modal closes on failure; auto profile regen is raced against a
  bounded wait; monitor coordinates render via Number(); luci.wloc mode-set
  reports real failures, restart_service/restart_gateway report restart
  failures, disable routes through the self-healing ctl path, the ctl
  envelope always emits valid JSON, fingerprint normalization strips all
  whitespace and echoes the gateway fingerprint safely, regen_ca backs up
  and rolls back the old CA and surfaces export failures; the dead duplicate
  overview.js was removed; proxy-health.json added to the read ACL and both
  divergent copies (rpcd plugin, ACL) were synced.
- Build/tests/docs: ELF-architecture gates on all runtime binary inputs of
  both builders (the x86_64 builder previously accepted an aarch64
  sing-box silently); the withdrawn-design self-referential
  traffic-isolation model/test was removed with a doc note; the orphan
  node-health-lock test joined verify.sh; docker matrix opkg branch creates
  /usr/sbin/ip for parity with the apk branch; contract-test lifecycle
  assertions made structural; RELEASE_PROCESS step 4/7 updated to the
  dual-variant six-asset reality; README/CHANGELOG bilingual r16 notes and
  wording fixes.

## Verification

- cargo test: 243 passed, 0 failed; cargo clippy --all-targets: 0 warnings;
  cargo fmt clean.
- All 14 tests/scripts suites pass, including the new arch-gate mocks.
- Python suite: 48 tests OK after removing the withdrawn-design model.
- Docker stub simulation re-run: empty scope withdraws, failed refresh
  withdraws, explicit stop is clean, successful install adds rule/route,
  and the guarded trap does not recurse.
- koalaman/shellcheck:stable clean on all changed scripts.
- ./scripts/ci/verify.sh: passed (see PR evidence).

## Failed attempts

- The first trap implementation used an inline `rc=$?` string that tripped
  shellcheck SC2154; rewritten as an explicit function that preserves the
  exit status.
- The new ELF gates initially failed three packaging tests whose stub
  binaries are shell scripts; the tests now ship a `file` mock (the pattern
  test-standalone-ax6s-package.sh already used) and the aarch64 mock was
  moved before the first builder invocation.

## Next executable steps

- Merge this PR, then rebuild the six r16 packages from the new commit
  (src/ changed, so aarch64/x86_64 cross builds are required), rerun the
  docker matrix, update the signed feed, tag v1.3.0-r16, publish the GitHub
  release, and perform the AX6S live upgrade with post-upgrade hygiene.

## Capabilities required for the next Agent

- GitHub CLI with access to wificalling-location-gateway and the
  Smthdagg-Repo-feeds repository.
- Docker for OpenWrt package, matrix, and feed verification.
- Local access to the feed signing key (`~/.zcode/keys/wloc-signing.key`).

## Security and privacy notes

- No credentials are included. The feed signing private key remains local
  and was not committed. Coordinate logging was removed from syslog as part
  of this hardening.
