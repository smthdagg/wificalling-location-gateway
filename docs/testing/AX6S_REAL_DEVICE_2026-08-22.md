# AX6S V2 integrated real-device evidence (redacted)

This is the redacted final AX6S evidence record. It contains no credentials,
MAC/IP addresses, node names, CA material, raw traffic, or precise locations.
The current acceptance run tests the independent integrated product: the
repository package contains both the WiFi Calling Gateway and WLOC modules,
while remaining independent from the separate Gateway 1.7 repository.

## Current integrated acceptance run

- Before testing, the previously installed WLOC package was stopped and
  removed, with modified UCI conffiles and WLOC/CA state backed up on the
  router. PassWall's provider was retained; no duplicate provider binary was
  installed.
- Package: `wificalling-location-gateway_2.0.0-34_aarch64_cortex-a53.ipk`
- SHA-256:

  ```text
  2ea44bed7bd20b97e5958d2b9cc6d6d39437865673241bd713aad6c497eebf91
  ```
- Startup result: supervisor `intercepting/ready`, `gateway=1`, `wloc=1`,
  `provider=1`, `redirect=1`.
- Health result: Gateway and WLOC both running; provider configuration valid;
  WLOC socket and redirect table/rule present; no WLOC error reported.
- Stop result: unified stop removed the WLOC table and Gateway table, stopped
  the in-repository Gateway monitor/sing-box and WLOC service, and left the
  PassWall-owned provider process running. Restart restored both services and
  the WLOC table.
- Scope result: the inspected ruleset contained no WLOC-owned UDP 500/4500
  interception. The Gateway nftables table remained Gateway-owned.
- Direct upgrade result: both modified conffiles were preserved, and the
  updater reported no stale transaction state after the integrated package
  migration. A legacy WLOC-only rollback package is rejected by the updater.
- Resource snapshot after restart: 242,260 KiB total memory, 8,004 KiB
  available, 15,680 KiB free on `/overlay`, and 16,824 KiB free in `/tmp`.
  This is a constrained but working AX6S baseline and should remain a release
  gate for package growth.

The remaining sections retain the earlier WLOC-only baseline and regression
evidence where useful; their historical package hashes are not the current
integrated release acceptance hash.

## Platform and migration

- Firmware family: ImmortalWrt 24.10.6, kernel 6.6.x, MediaTek MT7622,
  AArch64/cortex-a53.
- The installed `2.0.0-24` package was explicitly removed before the final
  migration test, as required for the small overlay. The modified UCI
  conffile and WLOC/CA state were backed up locally on the router first. The
  provider was retained. Later `2.0.0-25`, `2.0.0-26`, and `2.0.0-27` installs were
  package-upgrade checks; opkg preserved the modified UCI conffile and placed
  the package candidate beside it as `.opkg`.
- Provider packages: system sing-box and PassWall were retained; no second
  full-size sing-box binary was installed.
- Historical standalone package family: `wificalling-location-gateway`,
  `aarch64_cortex-a53`, firmware target `mediatek/mt7622`. Transactional update
  evidence used 2.0.0-17 -> 2.0.0-18 and an injected health-failure target
  2.0.0-19 -> automatic rollback to 2.0.0-18. The final rebuilt package
  `2.0.0-24` was first installed after removing the previous package and passed
  the service, API, provider, health, restart, target-metadata, and
  standalone-boundary checks. The final package `2.0.0-26` was installed after
  the audit fixes; the final package `2.0.0-27` SHA-256 was:

  ```text
  78c9a159a4e8da732ea7a8358b8f95eac2ed973ffa4dbd838e196fd5ff93479b
  ```
- The WLOC UCI profile and CA backups were taken before removal. The final
  provider path uses PassWall's persistent generated configuration under
  `/var/etc/passwall`; the provider process itself remains PassWall-owned.

## Resource observations

- Persistent storage: 87,620 KiB total; 15,016 KiB free after final package
  installation and configuration backup were present.
- Temporary storage: 121,128 KiB total; 25,548 KiB free after final
  installation.
- Memory: 242,260 KiB total; 18,436 KiB available in the final steady-state
  snapshot; no swap configured.
- WLOC RSS: 1,948 KiB, three threads.
- Reused PassWall sing-box RSS: 32,680 KiB, eight threads; no duplicate WLOC
  provider process was launched.
- The final AX6S package is approximately 1.4 MiB and the AArch64 WLOC service
  binary is 2,035,872 bytes; the OpenWrt cross-build gate also
  reported static AArch64 ELF with no dynamic dependency.

## Functional and recovery checks

| Check | Result |
|---|---|
| Normal remove-first migration after backup | pass |
| Unified supervisor startup | pass |
| Real `wloc_service` nftables table installed | pass |
| Policy route and scoped redirect installed | pass |
| WLOC health reports provider, socket, redirect, and profile state | pass |
| Profile CRUD with UCI-compatible underscore ID | pass |
| Profile manual/auto/manual persistence through the control API | pass |
| Service stop withdraws the WLOC table (fail-open) | pass |
| Service restart restores the WLOC table | pass |
| Three-profile runtime creates isolated `default/phone/tablet` tables | pass |
| Multi-profile handler activation keeps all enabled profiles intercepting | pass |
| Multi-profile status maps the explicit `default` profile to its own table | pass |
| Provider configuration check using persistent PassWall path | pass |
| Missing provider config after reboot keeps redirect withdrawn | pass |
| Delayed PassWall config generation auto-recovers WLOC | pass |
| Final reboot persistence window | pass |
| Signed component update preflight and 2.0.0-17 -> 2.0.0-18 update | pass |
| Health-gate failure rolls back 2.0.0-19 -> 2.0.0-18 | pass |
| Rollback removes the transaction directory and restores `current.version` | pass |
| Independent LuCI basic/devices/monitor/update assets present | pass |
| Final release candidate remove/install/restart on AX6S | pass |
| Final 2.0.0-26 restart settling and health recovery | pass |
| Controlled unified-supervisor stop withdraws redirect; start restores it | pass |
| Direct package install clears stale component-update status | pass |
| Package post-install uses init lifecycle without direct process kill | pass |
| Exact firmware target `mediatek/mt7622` matched package metadata | pass |
| Standalone package compatibility file and package boundary check | pass |
| V2 `Packages` and `Packages.gz` signatures verified by AX6S `usign` | pass |
| Final V2 device page, English source, and formal Chinese PO assets on AX6S | pass |
| Mobileconfig generation with unique private intermediate and cleanup | pass |
| Real iPhone WLOC traffic and packet capture | not run; no device fixture supplied |

## Findings fixed during the run

1. The final package initially lost the executable bit on the supervisor;
   packaging now asserts the executable mode.
2. A one-device profile was incorrectly treated as multi-profile readiness and
   could leave the supervisor waiting forever. The supervisor now enters the
   profile activation barrier only when more than one device profile exists.
3. A PassWall `/tmp` configuration path disappeared after reboot. The AX6S
   configuration now uses the persistent `/var/etc/passwall` path.
4. The supervisor previously checked only for a provider binary. It now checks
   the configured provider file and `sing-box check` before retaining or
   installing the redirect, and reports the provider state consistently.
5. UCI rejects hyphenated named sections and returns an error for deleting a
   missing optional option. Profile validation now matches UCI-safe IDs and
   optional deletes are idempotent.
6. The component updater relied on GNU tar auto-detection and `/usr/bin/opkg`.
   AX6S requires explicit gzip extraction and discovers `/bin/opkg`.
7. AX6S `opkg print-architecture` reports both `all` and `noarch`; the updater
   now ignores both and selects the real target architecture. The health gate
   also requires the WLOC-owned redirect table/rule, so it cannot commit an
   update while the service is running but traffic remains fail-open.
8. Package updates now reject a firmware target mismatch: AX6S reports
   `DISTRIB_TARGET='mediatek/mt7622'`, and the release package carries the same
   `X-WLOC-Target` value. Host regression coverage also rejects an `x86/64`
   package against the AX6S target.
9. The first real multi-profile run exposed a handler lifecycle mismatch: the
   shared ProfileRuntimeManager owned the nft table, while each profile handler
   still reported its redirect as absent. The handler now tracks a local
   logical redirect state, and the status projection distinguishes legacy
   singleton `default` from an explicit multi-profile `default` table.
10. Direct `opkg` installation could leave an older component-update result
    visible in LuCI. Package post-install now clears only the stale status
    projection; transactional update state is still written by the updater.

## Component update evidence

The real successful preflight reported `current_version=2.0.0-17`,
`target_version=2.0.0-18`, and 35,732 KiB available. The apply operation
completed with `phase=applied`, `reason=ready`, and current version
2.0.0-18. A second run deliberately made the health command fail only while
2.0.0-19 was installed. The updater reported:

```text
phase=rolled_back
reason=health_check_failed
target_version=2.0.0-19
current_version=2.0.0-18
```

After rollback, `opkg` and `current.version` both reported 2.0.0-18, the
transaction directory was absent, and the redacted health report showed
`wloc`, provider, and redirect all healthy. A hard power cut during opkg and a
real flash-full fault were not simulated; the repository interruption-recovery
test remains the evidence for that path.

## Host release evidence

The three-package release build and Docker matrix passed on 2026-08-22. The
matrix installed and started all four cases: AX6S/OpenWrt 24.10.5,
OpenWrt 24.10.8 x86_64, OpenWrt 25.12.3 x86_64, and iStoreOS 24.10.5 x86_64.
The exact host-side package hashes are recorded in the release staging
directory's `SHA256SUMS`; the final AX6S package hash is
`78c9a159a4e8da732ea7a8358b8f95eac2ed973ffa4dbd838e196fd5ff93479b` and was
verified on the router before installation. The AArch64 binaries were the
previously verified pinned-build outputs because the pinned cross-build image
was not cached locally during this final shell/UI/package-only rebuild.
Publication still requires
the release signing key and explicit external release approval.

The locally prepared V2 feed index was copied to a temporary AX6S verification
directory. Both detached signatures returned exit code 0 with the published
`wloc.pub`; no opkg source or installed package was changed by this check.

## Current acceptance status

AX6S integrated Gateway + WLOC runtime, remove-first migration, resource,
provider reuse, fail-open stop, unified restart, package target, UI/PO asset,
scoped nftables, and legacy-rollback rejection gates: **pass** for package
`2.0.0-34`.

The component update/rollback flow remains covered by the host and historical
AX6S transaction evidence below; it was not repeated during this package-only
runtime rerun.

The real-device WLOC client traffic path was not exercised because no test
device/fixture was supplied during this run. That is a separate functional
coverage item, not a dependency on another project. Signed feed publication
and external release approval remain release-process actions.
