# OpenWrt deployment scaffolding

This directory holds the packaging scaffold for the WLOC service control
daemon on OpenWrt 24.10 (Redmi AX6S / mt7622, AArch64).

## Layout

- `files/etc/init.d/wloc-service` — procd init script. Creates the
  root-owned mode-0700 `/var/run/wloc-service` directory and runs the daemon
  with the socket path env var. The daemon sets socket mode 0600.
- `files/etc/config/wloc-service` — UCI configuration skeleton. `enabled`,
  `node_ref`, `assigned_device`, `probe_interval`, and `geo_provider` are
  consumed by the runtime adapters as they land.
- `Makefile` — OpenWrt package definition. The Rust binary is produced by
  `scripts/ci/verify-rust-openwrt.sh` against the pinned OpenWrt 24.10
  toolchain and installed from `$(BIN_DIR)`.

## Deployment steps (on a prepared OpenWrt build host)

1. Build the AArch64 binary with the pinned cross-build script.
2. Copy the binary to `$(BIN_DIR)/wloc-service`.
3. `make package/wloc-service/compile` then `make package/wloc-service/install`.
4. Install the resulting `.ipk` on the router and start the service:
   `service wloc-service start`.

## Hard gates

- The daemon only serves the control API; no WLOC response patching, CA
  installation, traffic interception, or nftables redirect is implemented
  here. Those require the Phase 0 fixture and license gates to close.
- The socket is root-only (0600); no TCP management listener exists.
