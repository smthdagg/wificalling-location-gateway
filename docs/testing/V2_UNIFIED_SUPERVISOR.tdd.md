# V2 unified supervisor TDD evidence

## Scope

This evidence covers Issue #33's unified Gateway/WLOC lifecycle slice. The
device-profile, LuCI, update, migration, and AX6S live-RSS slices remain
follow-up work from the V2 task breakdown.

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

Result: PASS. The run included 69 Python tests, Rust all-target tests, Rust
line coverage of 81.29%, OpenWrt cross-build/resource gates, release package
tests, standalone AX6S package tests, JavaScript tests, secret scanning,
formatting, and dependency audit. Cargo audit reported no advisories; it did
report the pre-existing duplicate `socket2` and `windows-sys` lock entries.

## Guarantees and gaps

| Guarantee | Test or evidence |
|---|---|
| Redirect installation is after child start and health check | `tests/scripts/test-unified-supervisor.sh`; `tests/service_supervisor.rs` |
| WLOC fault withdraws the WLOC redirect and leaves Gateway passthrough alive | `openwrt/files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh` failure paths plus shell static gate |
| Cleanup uncertainty is visible as `CleanupUnsafe` | `tests/service_supervisor.rs:cleanup_failure_is_not_reported_as_stopped` |
| Stable Gateway nftables namespace and UDP 500/4500 are not directly edited by WLOC cleanup | shell static gate and dedicated `wloc_service` stop mock |
| Release packaging enables the unified entry point | `tests/scripts/test-openwrt-release-packaging.sh` and `tests/scripts/test-standalone-ax6s-package.sh` |
| No claim is made for live AX6S RSS, procd behavior, or multi-device UI in this slice | Requires hardware/package-install acceptance in the next issue |
