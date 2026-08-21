# AX6S V2 real-device evidence (redacted)

This is the redacted final AX6S evidence record. It contains no credentials,
MAC/IP addresses, node names, CA material, raw traffic, or precise locations.
The tested product is the standalone WLOC service; no Wi-Fi Calling Gateway
package, UCI file, init script, or runtime dependency was installed.

## Platform and migration

- Firmware family: ImmortalWrt 24.10.6, kernel 6.6.x, MediaTek MT7622,
  AArch64/cortex-a53.
- Old WLOC package: the installed `2.0.0-1` release candidate was stopped,
  disabled, and removed before the rebuilt `2.0.0-1` package was installed, as
  required for the small overlay. The modified UCI conffile was preserved by
  opkg and the pre-install UCI/CA backups remain under the router's temporary
  backup directory.
- Provider packages: system sing-box and PassWall were retained; no second
  full-size sing-box binary was installed.
- Tested standalone package family: `wificalling-location-gateway`,
  `aarch64_cortex-a53`, firmware target `mediatek/mt7622`. The real-device
  baseline was 2.0.0-14; transactional update evidence used 2.0.0-17 -> 2.0.0-18
  and an injected health-failure target 2.0.0-19 -> automatic rollback to
  2.0.0-18. The rebuilt release candidate `2.0.0-1` was then installed after
  removing the prior package and passed the service, API, provider, health,
  restart, target-metadata, and standalone-boundary checks.
- The WLOC UCI profile and CA backups were taken before removal. The final
  provider path uses PassWall's persistent generated configuration under
  `/var/etc/passwall`; the provider process itself remains PassWall-owned.

## Resource observations

- Persistent storage: 87,620 KiB total; 13,836 KiB free after the rebuilt
  package and configuration backup were present.
- Temporary storage: 121,128 KiB total; 32,780 KiB free after installation.
- Memory: 242,260 KiB total; 23,248 KiB available in the final steady-state
  snapshot; no swap configured.
- WLOC RSS: 1,952 KiB, three threads.
- Reused PassWall sing-box RSS: 30,176 KiB, eight threads; no duplicate WLOC
  provider process was launched.
- The final AX6S package itself is 1,445,912 bytes and the AArch64 WLOC service
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
| Provider configuration check using persistent PassWall path | pass |
| Missing provider config after reboot keeps redirect withdrawn | pass |
| Delayed PassWall config generation auto-recovers WLOC | pass |
| Final reboot persistence window | pass |
| Signed component update preflight and 2.0.0-17 -> 2.0.0-18 update | pass |
| Health-gate failure rolls back 2.0.0-19 -> 2.0.0-18 | pass |
| Rollback removes the transaction directory and restores `current.version` | pass |
| Independent LuCI basic/devices/monitor/update assets present | pass |
| Final release candidate remove/install/restart on AX6S | pass |
| Exact firmware target `mediatek/mt7622` matched package metadata | pass |
| Standalone package compatibility file and package boundary check | pass |
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
directory's `SHA256SUMS`; the rebuilt AX6S package hash is
`90762e2453ffae11341fef6caa42bef379ba52a9599d5aeb73bcc0a2952f231f` and was
verified again on the router before installation. Publication still requires
the release signing key and explicit external release approval.

## Acceptance status

AX6S standalone runtime, migration, resource, provider, reboot, fail-open,
mobileconfig, UI/PO asset, release-candidate installation, and transactional
health-rollback gates: **pass**.

The real-device WLOC client traffic path was not exercised because no test
device/fixture was supplied during this run. That is a separate functional
coverage item, not a dependency on another project. Signed feed publication
and external release approval remain release-process actions.
