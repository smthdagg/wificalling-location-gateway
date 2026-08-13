# WLOC location-mode switch TDD evidence

## User journey

As a LuCI administrator, I can switch WLOC between Auto and Manual without a race between UCI persistence, LuCI apply, and the runtime control request.

## RED evidence

- Test: `tests/js/wloc_mode_switch.test.js`, invoked by `tests/test_wloc_luci_mode.py`.
- Command: `python3 -m unittest tests.test_wloc_luci_mode -v`.
- Result before the fix: FAIL with `mode switch must return its Promise`.
- Checkpoint: `d5510c3 test: reproduce WLOC mode switch race`.

The reproducer also showed that the old handler called `ui.changes.apply()` and `geo-set` before the asynchronous UCI save completed.

AX6S validation exposed two additional RED cases that the browser-only test did
not model: the LuCI apply restarted the daemon after the control request, and a
pre-package hand deployment left an orphan process holding the proxy port while
its control socket refused connections.

## GREEN evidence

The handler now sends one `mode-set` operation through the already-authorized
`luci.wloc/ctl` bridge. The server-side operation:

1. validates `auto` or `manual` and requires both manual coordinates;
2. applies `geo-set` or `geo-clear` to the live daemon without a browser-side
   UCI apply or restart;
3. commits `geo_source` and manual coordinates only from the root-side bridge;
4. if the control socket is unavailable, removes only an orphaned
   `wloc-service`, returns lifecycle ownership to `procd`, and retries for at
   most 10 seconds;
5. returns the complete Promise and reports an exhausted recovery as one
   user-visible error.

This removes the competing browser save/apply/restart sequence that caused the
stale-socket race. The AX6S package installs the current static AArch64 service
and control binaries, preserves `/etc/config/wloc-service` as a conffile, and
starts the service under `procd`.

Manual mode without stored coordinates is rejected before persistence with an actionable message.

| Guarantee | Test | Result |
|---|---|---|
| Manual switch uses one atomic root-side `mode-set` operation | `verifyManualSwitch` | PASS |
| Auto switch uses the same bounded root-side operation | `verifyAutoSwitch` | PASS |
| Missing manual coordinates do not persist or reach runtime control | `verifyManualSwitchWithoutCoordinates` | PASS |
| Both OpenWrt source and LuCI package copies behave identically | test source matrix | PASS |

## Full verification

- `./scripts/ci/verify.sh`: PASS.
- Python discovery: 46 tests, PASS.
- Rust line coverage: 80.25%.
- JavaScript syntax checks: PASS.
- Secret scan and dependency policy checks: PASS.

## AX6S validation

- Target: Redmi AX6S, ImmortalWrt 24.10.6, MediaTek MT7622/AArch64.
- Installed package: `0.1.0-2-ax6s14` (full LuCI + static service + control
  binaries).
- Reproduced the original Auto-to-Manual journey from the real LuCI page.
- No `Mode switch failed` notification and no `Connection refused` response.
- Final monitor state: `intercepting`, `Manual`, GPS
  `22.319300 / 114.169400`, Geo `fresh`.
- The router was left in that original manual-location state.
