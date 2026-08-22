# V2 requirement audit

Date: 2026-08-22  
Scope: integrated `wificalling-location-gateway` Gateway + WLOC project only.

This matrix records the seven known design questions and the evidence used for
each conclusion. The separate Wi-Fi Calling Gateway 1.7 repository is outside
the product boundary and is not a dependency or test fixture; the
in-repository Gateway module is part of the product and package.

| # | Requirement / question | Result | Evidence |
|---|---|---|---|
| 1 | Component update is an independent page | PASS | Independent LuCI update asset/controller, package/update contract tests, and AX6S asset check. |
| 2 | Update must match the device firmware | PASS | Updater validates package format, architecture, OpenWrt major version, and exact `DISTRIB_TARGET`; target mismatch is covered by `tests/scripts/test-component-update.sh`. Release builder emits `mediatek/mt7622` for AX6S and `x86/64` compatibility metadata for x86 packages. |
| 3 | Fixed/Auto follow semantics are unambiguous | PASS | A profile owns its explicit node reference. Auto mode follows that profile node's exit; Fixed/manual mode stores the profile's own coordinates. There is no hidden global “follow another gateway” relationship. See ADR 0003 and the profile model tests. |
| 4 | Manual WLOC writes to the selected device profile | PASS | Device profile CRUD and manual/auto/manual persistence tests exercise the same profile record; the device page is the ownership surface, while the basic page exposes global defaults/summary only. |
| 5 | LuCI language policy is consistent | PASS | English is the source language for new UI strings; Chinese translations are provided through the formal `po/zh_Hans` catalog. Contract tests check both Gateway and WLOC page assets and the translation catalog. |
| 6 | The service is truly unified | PASS | One supervisor, one package, shared diagnostics/update/rollback, and one management surface coordinate the in-repository Gateway and WLOC modules while preserving their scoped data planes. |
| 7 | The repository is independent | PASS | ADR 0004, package/release scans, and package metadata contain no install/configure/runtime dependency on the separate Wi-Fi Calling Gateway 1.7 repository. The in-repository Gateway module remains included and managed. |

## Newly fixed audit gap

The first implementation validated architecture and OpenWrt release family but
could accept a package built for another firmware target. The updater now reads
`DISTRIB_TARGET` from `/etc/openwrt_release` and rejects a package whose
`X-WLOC-Target` differs. The test was written first (`1172a03`) and the fix was
implemented in `b68ff9d`.

## Remaining open evidence

These are test-environment gaps, not accepted design shortcuts:

- real iPhone WLOC traffic and packet capture;
- hard power interruption during package replacement;
- physical flash-full fault injection;
- publication of the signed feed, tag, and external release approval.

The repository interruption-recovery test and AX6S transactional rollback test
cover the software paths for the latter two update failure classes, but they do
not replace physical fault injection.
