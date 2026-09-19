# WLOC notification dismissal TDD evidence

## User journey

As a LuCI administrator, I can apply a WLOC preset without a sticky success
dialog, and I can dismiss an actionable error directly from the page.

## RED evidence

- Test: `tests/js/wloc_notification.test.js`.
- Command: `node tests/js/wloc_notification.test.js`.
- Result before the fix: FAIL because success used the stock LuCI notification
  and errors had no page-owned dismissal behavior.
- Checkpoint: `2eedfc6 test: reproduce WLOC notification dismissal bug`.

## GREEN evidence

The WLOC view now suppresses non-actionable success banners. Errors render in a
local alert with an explicit button that prevents the default action and removes
that exact alert node. Both packaged LuCI source copies use the same behavior.

| Guarantee | Test | Result |
|---|---|---|
| Applying a successful preset creates no blocking success banner | `wloc_notification.test.js` success case | PASS |
| Errors do not use the theme-dependent global notification | `wloc_notification.test.js` error case | PASS |
| The error close button removes the visible alert | `wloc_notification.test.js` close case | PASS |
| Both OpenWrt source and LuCI package copies behave identically | source matrix | PASS |

## Full verification

- `./scripts/ci/verify.sh`: PASS.
- Python discovery: 48 tests, PASS.
- Rust line coverage: 81.51%.
- JavaScript regression guards, secret scan, packaging, and dependency policy
  checks: PASS.
- ShellCheck was not installed locally; the repository gate reported the
  existing warning and CI remains responsible for that lint step.

## Known gap

The change was verified with the repository's LuCI view harness; a live browser
click on AX6S still requires deploying the next package to the router.
