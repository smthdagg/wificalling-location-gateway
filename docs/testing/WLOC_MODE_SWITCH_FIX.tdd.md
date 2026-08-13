# WLOC location-mode switch TDD evidence

## User journey

As a LuCI administrator, I can switch WLOC between Auto and Manual without a race between UCI persistence, LuCI apply, and the runtime control request.

## RED evidence

- Test: `tests/js/wloc_mode_switch.test.js`, invoked by `tests/test_wloc_luci_mode.py`.
- Command: `python3 -m unittest tests.test_wloc_luci_mode -v`.
- Result before the fix: FAIL with `mode switch must return its Promise`.
- Checkpoint: `d5510c3 test: reproduce WLOC mode switch race`.

The reproducer also showed that the old handler called `ui.changes.apply()` and `geo-set` before the asynchronous UCI save completed.

## GREEN evidence

The handler now serializes:

1. save `geo_source` to UCI;
2. wait for LuCI changes to apply;
3. let the restarted daemon load Auto or Manual from UCI;
4. return the complete Promise and convert rejected save/apply operations into one user-visible error.

The UI deliberately does not call `geo-set` or `geo-clear` after apply. LuCI apply restarts the service, and calling the old control socket in that restart window produces `Connection refused`. Startup already reads `geo_source`, `manual_lat`, and `manual_lon`, so a second runtime request is both redundant and unsafe.

Manual mode without stored coordinates is rejected before persistence with an actionable message.

| Guarantee | Test | Result |
|---|---|---|
| Manual switch saves and applies without racing the restarted control socket | `verifyManualSwitch` | PASS |
| Auto switch saves and applies without calling the stale control socket | `verifyAutoSwitch` | PASS |
| Missing manual coordinates do not persist or reach runtime control | `verifyManualSwitchWithoutCoordinates` | PASS |
| Both OpenWrt source and LuCI package copies behave identically | test source matrix | PASS |

## Full verification

- `./scripts/ci/verify.sh`: PASS.
- Python discovery: 46 tests, PASS.
- Rust line coverage: 80.32%.
- JavaScript syntax checks: PASS.
- Secret scan and dependency policy checks: PASS.

Browser/device validation on the AX6S remains the deployment follow-up; the regression test exercises the real LuCI page source with controlled asynchronous save and apply operations.
