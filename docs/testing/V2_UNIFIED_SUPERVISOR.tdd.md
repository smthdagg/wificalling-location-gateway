# V2 unified supervisor TDD evidence

## Scope

This evidence started with Issue #33's unified WLOC lifecycle slice.
The current V2 integration branch extends it with UCI-backed device profiles,
LuCI management/monitoring, bounded diagnostics, component update/rollback
contracts, and shared sing-box provider resolution. AX6S RSS, migration,
reboot, fail-open, and transactional health-rollback observations are recorded
in the redacted real-device evidence; hard power-cut and flash-full faults
remain hardware gaps.

## RED/GREEN checkpoints

| Behavior | RED evidence | GREEN evidence |
|---|---|---|
| A failed cleanup is not reported as cleanly stopped | `cargo test --test service_supervisor cleanup_failure_is_not_reported_as_stopped` failed to compile because `CleanupUnsafe` was not implemented | The same target passed; the full supervisor integration test passed with 8 tests |
| Legacy WLOC cannot install redirect before unified health checks | `./tests/scripts/test-unified-supervisor.sh` exited 1 after assertions for `WLOC_SKIP_REDIRECT` and fail-open cleanup were added | `unified supervisor shell tests passed` |
| Runtime control delegates only the WLOC-owned redirect | `cargo test --bin wloc-service openwrt_runtime_delegates_only_component_redirect_actions` failed to compile because `OpenWrtRuntime` was absent | The test passed and verified `start` then `stop` against a fake helper |
| Unified mode does not enable independent child respawn | The shell test exited 1 before supervised-mode guards were added | `unified supervisor shell tests passed`; release and standalone package tests passed |

## Full verification

Command:

```sh
./scripts/ci/verify.sh
```

Result: PASS. The run included 83 Python tests, Rust all-target tests, Rust
line coverage of 80.11%, OpenWrt cross-build/resource gates, release package
tests, standalone AX6S package tests, JavaScript tests, secret scanning,
formatting, and dependency audit. Cargo audit reported no advisories; it did
report the pre-existing duplicate `socket2` and `windows-sys` lock entries.

## Guarantees and gaps

| Guarantee | Test or evidence |
|---|---|
| Redirect installation is after child start and health check | `tests/scripts/test-unified-supervisor.sh`; `tests/service_supervisor.rs` |
| WLOC fault withdraws the WLOC redirect and leaves unrelated router traffic outside the WLOC table | `openwrt/files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh` failure paths plus shell static gate |
| Cleanup uncertainty is visible as `CleanupUnsafe` | `tests/service_supervisor.rs:cleanup_failure_is_not_reported_as_stopped` |
| Unrelated nftables namespaces and UDP 500/4500 are not directly edited by WLOC cleanup | shell static gate and dedicated `wloc_service` stop mock |
| Release packaging enables the unified entry point | `tests/scripts/test-openwrt-release-packaging.sh` and `tests/scripts/test-standalone-ax6s-package.sh` |
| AX6S RSS, procd behavior, migration, and rollback timing are evidenced | `docs/testing/AX6S_REAL_DEVICE_2026-08-22.md`; hard power-cut/flash-full remain open |
