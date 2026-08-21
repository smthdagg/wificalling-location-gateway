# AX6S V2 real-device evidence (redacted)

This is the redacted final AX6S evidence record. It contains no credentials,
MAC/IP addresses, node names, CA material, raw traffic, or precise locations.
The tested product is the standalone WLOC service; no Wi-Fi Calling Gateway
package, UCI file, init script, or runtime dependency was installed.

## Platform and migration

- Firmware family: ImmortalWrt 24.10.x, kernel 6.6.x, MediaTek MT7622,
  AArch64/cortex-a53.
- Old WLOC package: `2.0.0-10`, stopped/disabled and removed before the final
  package was installed, as required for the small overlay.
- Provider packages: system sing-box and PassWall were retained; no second
  full-size sing-box binary was installed.
- Tested standalone package: `wificalling-location-gateway` 2.0.0-14,
  `aarch64_cortex-a53`.
- The WLOC UCI profile and CA backups were taken before removal. The final
  provider path uses PassWall's persistent generated configuration under
  `/var/etc/passwall`; the provider process itself remains PassWall-owned.

## Resource observations

- Persistent storage: 87,620 KiB total; 15,868 KiB free after the final
  package was installed.
- Temporary storage: 121,128 KiB total; 45,672 KiB free after installation.
- Memory: 242,260 KiB total; 36,156 KiB available in the final steady-state
  snapshot; no swap configured.
- WLOC RSS: 1,948 KiB, three threads.
- Reused PassWall sing-box RSS: 30,176 KiB, eight threads; no duplicate WLOC
  provider process was launched.
- The final package itself is 1,444,108 bytes and the AArch64 WLOC service
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
| Independent LuCI basic/devices/monitor/update assets present | pass |
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

## Acceptance status

AX6S standalone runtime, migration, resource, provider, reboot, and fail-open
gates: **pass**.

The real-device WLOC client traffic path was not exercised because no test
device/fixture was supplied during this run. That is a separate functional
coverage item, not a dependency on another project. Signed feed publication
and external release approval remain release-process actions.
