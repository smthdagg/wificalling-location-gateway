# Agent handoff: Issue 38

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: openwrt,test,security,docs
- Branch: codex/issue-38-v2-logs-support-codex-v2-lead-20260821091436-dbe9e0a4
- Final local checkpoint before handoff: 4c9a481
- Credentials included: no

## Objective

Implement V2 bounded structured diagnostics for the unified Gateway/WLOC
service: one privacy-safe event envelope, bounded recent-history logs,
diagnostic support bundle generation, normal/debug observability controls, and
packaging/UI integration suitable for small OpenWrt gateways.

## Completed

- Unified WLOC service, MITM rewrite, and Gateway diagnostic events around the
  documented JSONL envelope with timestamp, component, profile scope, severity,
  event code, stable message, and bounded fields.
- Added one shared Rust event writer with 64 KiB file and 2 KiB record limits;
  rotation keeps newest complete records and drops oversized events.
- Removed precise coordinates and device material from WLOC rewrite history;
  rewrite events retain only response byte counters.
- Kept Gateway legacy pipe compatibility while making JSON event retention
  schema-aware; JSON events are globally bounded because they intentionally
  contain no device key.
- Added privacy-safe support bundle collection through root-only rpcd method:
  manifest and availability booleans plus whitelisted event codes only; raw
  health/status/config/log material is not copied.
- Added support-bundle storage cap (default 64 KiB, hard maximum 128 KiB),
  mode 0600 output, collection lock, private temporary archive, failure-safe
  replacement, symlink rejection, and a 600-second operational expiry field.
- Added the helper to canonical Makefile, standalone AX6S/LuCI package, and
  release package paths.
- Added LuCI health-page support bundle action, structured event parsing, ACL
  separation, shell regression coverage, packaging contract coverage, and
  operations documentation.

## Files changed

- `src/diagnostics.rs`
- `src/lib.rs`
- `src/app.rs`
- `src/mitm/proxy.rs`
- `openwrt/files/usr/libexec/wificalling-gateway/monitor.sh`
- `openwrt/files/usr/sbin/wloc-support-bundle.sh`
- `openwrt/Makefile`
- `scripts/build-luci-ipk.sh`
- `docs/operations/V2_STRUCTURED_DIAGNOSTICS.md`
- `tests/mitm_proxy.rs`
- `tests/scripts/test-support-bundle.sh`
- `tests/test_v2_diagnostics_contract.py`

The LuCI, rpcd, ACL, release-copy, and verification changes from the initial
V2-06 implementation are included in the parent commit `bf4a0bf`.

## Verification

| Command | Result | Evidence |
|---|---|---|
| `./scripts/ci/verify.sh` | Passed | 78 Python tests, all Rust targets, OpenWrt/AX6S/release packaging, structured logs, support bundle, secret scan, cargo audit |
| `cargo llvm-cov --workspace --all-targets --locked --fail-under-lines 80` | Passed | 80.11% total Rust line coverage |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed | No warnings |
| `cargo test --test mitm_proxy wloc_response_is_patched_through_the_proxy` | Passed | Rewrite event schema and privacy assertions |
| `sh tests/scripts/test-structured-logs.sh` | Passed | Gateway schema, privacy, and byte cap |
| `sh tests/scripts/test-support-bundle.sh` | Passed | Privacy, cap, and symlink rejection |
| `python3 tests/test_v2_diagnostics_contract.py` | Passed | RPC/ACL/UI and all package-path contract |
| `git diff --check` | Passed | No whitespace errors |

## Failed attempts

- The first implementation left MITM rewrite events on the old unbounded
  `type/time/latitude/longitude` path; independent review caught it. The fix
  moved bounded writing into `src/diagnostics.rs`, removed coordinates, and
  updated the integration test.
- The first support-bundle implementation streamed tar output directly to the
  final path and could leave partial output on archive failure. It now builds
  privately and moves only a verified bounded archive into place.
- The first packaging pass updated release packaging but missed the canonical
  OpenWrt Makefile and standalone AX6S/LuCI package path. Both are now covered
  by code and contract tests.
- Commit hook reported `lefthook` unavailable in PATH; the commit itself
  succeeded and the repository verification script passed all required gates.

## Warnings and non-blocking notes

- `cargo audit` reports existing duplicate `socket2` and `windows-sys` lockfile
  entries; advisories, bans, licenses, and sources all pass.
- Gateway JSON diagnostic events intentionally do not contain a device key, so
  their record-count retention is global; legacy pipe records retain the
  existing per-device behavior.
- The support bundle uses a fixed default path for LuCI compatibility and
  serializes generation with `/tmp/wloc-support-bundle.lock`; a stale lock
  after an abnormal power loss requires manual cleanup on the device.

## Unresolved decisions and blockers

- Component update, compatibility gate, and rollback: Issue #39.
- AX6S memory/storage/CPU gates and real-device evidence: Issue #40.
- Migration rehearsal, complete integration acceptance, and V2 release: Issue #41.

## Capabilities required for the next Agent

- `openwrt`, `test`, `security`, `docs`, and independent review of shell
  packaging and root-only rpcd boundaries.

## Environment assumptions

- The two OpenWrt source trees are packaged separately and both remain covered
  by contract tests.
- `/tmp` exists and supports `mktemp`, directory locks, gzip tar, and atomic
  rename on the target OpenWrt image.
- `wloc-health.sh` is an optional local producer; an unavailable or failed
  producer is reported as unavailable rather than healthy.

## Security and privacy notes

- No credentials, tokens, private keys, raw traffic, device addresses, or
  precise coordinates are emitted in structured event history or support
  bundle content.
- Runtime interception scope remains one assigned device, the two exact Apple
  WLOC hostnames, and TCP 443; UDP 500/4500 and the stable Gateway nftables
  namespace remain untouched.
- `support_bundle` is write-only in both ACL sources; status/health remains
  read-only.

## Next executable steps

1. Run `scripts/agent-handoff.sh 38 codex-v2-lead openwrt,test,security,docs`.
2. Push the branch and open a PR against `main` with `Closes #38`, this handoff
   capsule, verification evidence, risk, and rollback notes.
3. Require independent review and all GitHub checks before merge.
4. After merge, take Issue #39 for component update, compatibility gating, and
   rollback, then continue through AX6S resource evidence and final V2 release.
