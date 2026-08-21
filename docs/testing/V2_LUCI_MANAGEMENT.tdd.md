# V2 LuCI unified management contract

## Scope

Issue #37 adds the first unified management surface for the Gateway/WLOC
runtime. The page is intentionally small-router friendly: it uses UCI for
staged edits, one bounded health request every 15 seconds, and a redacted
runtime projection. It does not expose node credentials, raw traffic, or
precise runtime probe data.

## User journeys

1. Open `WLOC Device Profiles` and see basic service settings, Gateway/WLOC
   summary, device profiles, node mode, WLOC auto/manual mode, enablement,
   and `phase (reason_code)` state.
2. Edit one or more rows, add a row, or stage a deletion. No service restart
   or persistent commit occurs at this point.
3. Press `Apply & restart`. The page validates profile count, IDs, duplicate
   device bindings, bounded strings, node/location modes, coordinates, probe
   interval, provider, and private-IPv4/unicast-MAC device binding before
   calling the single save/apply/restart path. Device duplicate detection
   canonicalizes IPv4 text and MAC separators.
4. Enter an invalid value and apply. Validation stops before `uci.save()`;
   the running configuration is therefore unchanged.
5. Leave the page open. Status refreshes at 15-second intervals without
   rebuilding the input table, so staged edits are not lost.

## Automated evidence

- `tests/test_v2_ui_contract.py` checks both OpenWrt source copies, required
  settings/status/apply strings, bounded-input guards, and the low-frequency
  polling contract.
- `node --check` parses both LuCI page copies.
- `./scripts/ci/verify.sh` remains the release gate and also covers the
  profile dispatcher, supervisor, package, ACL, OpenWrt, and AX6S checks.

## Deliberate boundaries

- The legacy WLOC page remains available for CA/profile export and advanced
  location tools during the V2 migration. The new page owns profile lifecycle
  and the unified apply boundary.
- Transactional validation is enforced before persistence. A future backend
  apply RPC may make commit/restart rollback atomic across daemon failures;
  that is tracked with the broader V2 release work rather than silently
  pretending the LuCI client can roll back a committed UCI transaction.
- Logs, support bundles, component update/rollback, and resource gates are
  separate V2 issues (#38–#40) and must not be implemented by adding high-rate
  polling or unbounded browser-side history here.
- The `restart_unified` RPC is write-only in both packaged ACL sources; the
  canonical and package ACL files are tested byte-for-byte to prevent a
  permission drift during packaging.
