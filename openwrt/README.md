# OpenWrt packaging and runtime integration

This directory contains the production OpenWrt integration for release 1.0:
the Rust daemon/control client, procd/UCI lifecycle, precise WLOC network
helpers, rpcd/LuCI UI, and package definitions.

## Layout

- `files/etc/init.d/wloc-service` — procd init script. Creates the
  root-owned mode-0700 `/var/run/wloc-service` directory and runs the daemon
  with the socket path env var. The daemon sets socket mode 0600.
- `files/etc/config/wloc-service` — UCI configuration skeleton. `enabled`,
  `node_ref`, `assigned_device`, `probe_interval`, and `geo_provider` are
  consumed by the runtime adapters as they land.
- `Makefile` — component package definition for feed development. The Rust binary is produced by
  `scripts/ci/verify-rust-openwrt.sh` against the pinned OpenWrt 24.10
  toolchain and installed from `$(BIN_DIR)`.

## Release packaging

End users install one architecture-specific package named
`wificalling-location-gateway`. AX6S uses
`wificalling-location-gateway_1.0.0-1_aarch64_cortex-a53.ipk`; x86-64 24.x
uses the integrated IPK and 25.x uses the native APK v3. Both UCI paths are
declared as conffiles, so direct upgrade/reinstall preserves configuration.

The release builder composes a pinned, verified stable integrated 1.2.x IPK at build time;
the source repository remains isolated and does not vendor the Gateway source.

## Hard gates

- Interception is restricted to the assigned test device, exact authorized
  Apple hosts, and TCP 443. UDP 500/4500 and the Gateway nftables table are
  outside the WLOC data plane.
- The socket is root-only (0600); no TCP management listener exists.
