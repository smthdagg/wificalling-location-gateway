# OpenWrt packaging and runtime integration

This directory contains the production OpenWrt integration for the V2 unified
release:
the Rust daemon/control client, procd/UCI lifecycle, precise WLOC network
helpers, rpcd/LuCI UI, and package definitions.

## Layout

- `files/etc/init.d/wloc-service` — procd init script. Creates the
  root-owned mode-0700 `/var/run/wloc-service` directory and runs the daemon
  with the socket path env var. The daemon sets socket mode 0600.
- `files/etc/config/wloc-service` — UCI configuration skeleton. `main` holds
  shared settings and each `device` section owns one assigned device, node
  policy, WLOC mode, enablement and runtime status.
- `Makefile` — component package definition for feed development. The Rust binary is produced by
  `scripts/ci/verify-rust-openwrt.sh` against the pinned OpenWrt 24.10
  toolchain and installed from `$(BIN_DIR)`.

## Release packaging

End users install one architecture-specific package named
`wificalling-location-gateway`. AX6S uses the architecture-specific
`wificalling-location-gateway_2.0.0-1_aarch64_cortex-a53.ipk`; x86-64 24.x
uses the integrated IPK and 25.x uses the native APK v3. Both UCI paths are
declared as conffiles, so direct upgrade/reinstall preserves configuration.

The release builder composes a pinned, verified Gateway IPK at build time;
the source repository remains isolated and does not vendor the Gateway source.

## Hard gates

- Interception is restricted to the assigned test device, exact authorized
  Apple hosts, and TCP 443. UDP 500/4500 and the Gateway nftables table are
  outside the WLOC data plane.
- The socket is root-only (0600); no TCP management listener exists.

## Small-gateway resource contract

The package installs a machine-readable V2 budget at
`/usr/share/wificalling-location-gateway/resource-budget.conf`. CI enforces
the combined release-binary ceiling and supplied package/runtime reports;
release packaging invokes the package check for each output artifact. AX6S
idle RSS, peak RSS, CPU/startup, and update/rollback measurements must be
recorded with the redacted evidence template before persistent deployment is
accepted.

## Sing-box provider reuse

The integrated package does not add a second full-size sing-box dependency.
The runtime resolver prefers the tested `sing-box-tiny`/`sing-box-lite` or an
existing PassWall sing-box binary, while the unified supervisor retains
ownership of the process it starts. See
[`V2_SINGBOX_RUNTIME.md`](../docs/operations/V2_SINGBOX_RUNTIME.md) and keep
the selected provider installed when removing the old AX6S application package.
