# AX6S V2 real-device evidence (redacted)

This is a staging evidence record for the Redmi AX6S run. It contains no
credentials, MAC/IP addresses, node names, CA material, raw traffic, or
precise locations. The integrated package was built with a temporary local
Gateway fixture for runtime testing; it is not the final signed release
Gateway payload.

## Platform and migration

- Firmware family: ImmortalWrt 24.10.x, kernel 6.6.x, MediaTek MT7622,
  AArch64/cortex-a53.
- Old application package: 1.2.x, stopped/disabled and removed before the
  integrated package was installed.
- Provider packages: system sing-box and PassWall were retained; no second
  full-size sing-box binary was installed.
- Tested integrated package: `wificalling-location-gateway` 2.0.0-10,
  `aarch64_cortex-a53`.
- Configuration and CA backups were taken before removal; the live UCI
  configuration hashes remained unchanged through migration and update.

## Resource observations

- Persistent storage: approximately 85 MiB total; about 15 MiB free after the
  final test package was installed.
- Temporary storage: approximately 35 MiB free after installation.
- Memory: approximately 242 MiB total; approximately 15 MiB available in the
  final steady-state snapshot; no swap configured.
- WLOC RSS: approximately 1.2 MiB, three threads.
- Two shared sing-box processes: approximately 29 MiB and 33 MiB RSS.
- The observed memory pressure is acceptable for this staging device, but
  final release acceptance still requires a measured CPU sample and the real
  validated Gateway 1.7 artifact.

## Functional and recovery checks

| Check | Result |
|---|---|
| Normal remove-first migration after backup | pass |
| Unified supervisor startup | pass |
| Real `wloc_service` nftables table installed | pass |
| Policy route installed | pass |
| Hosts marker written to both runtime files | pass |
| Profile CRUD with UCI-compatible underscore ID | pass |
| Hyphenated profile ID rejected before UCI mutation | pass |
| WLOC disable withdraws its table while Gateway table remains | pass |
| WLOC enable restores its table | pass |
| Signed update preflight: hash/manifest/architecture | pass |
| Signed update apply and health validation | pass |
| Simulated interrupted update and `recover` | pass |
| Gateway and WLOC health after recovery | pass |
| Real Wi-Fi Calling call and carrier registration | pending |
| Real client HTTPS isolation and packet capture | pending |
| Reboot persistence window | pending |

## Findings fixed during the run

1. The redirect helper expanded its default hosts list as one path containing
   a space. The service could report ready while no WLOC redirect was
   installed. The default and override paths now use explicit file iteration.
2. UCI rejects hyphenated named sections and returns an error for deleting a
   missing optional option. Profile validation now matches UCI-safe IDs and
   optional deletes are idempotent.
3. The component updater relied on GNU tar auto-detection and `/usr/bin/opkg`.
   AX6S requires explicit gzip extraction and discovers `/bin/opkg`.
4. The health report is informational and does not fail its process. The
   updater now validates the redacted health fields and waits within a bounded
   timeout for asynchronous supervisor startup.

## Acceptance status

AX6S runtime and migration gates: **staging pass**.

V2.0 release: **not yet accepted**. Remaining gates are the real validated
Gateway 1.7 payload, signed release artifacts/checksums, CPU measurement,
reboot persistence, real Wi-Fi Calling traffic, client isolation evidence,
and final rollback/release review.
