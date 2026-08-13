# OpenWrt release packaging and Docker matrix

## Packaging boundary

The reference Wi-Fi Calling Gateway can publish one `all.ipk` and one
`noarch.apk` because its payload is Lua, JavaScript, and shell. WLOC adds two
Rust ELF executables, so the runtime package must name the real OpenWrt CPU
architecture. Marking an AArch64 or x86-64 ELF as `all` is invalid and can
install an unusable binary on another router.

The release therefore contains two layers for each package-manager generation.
The Docker release builder in this document is deliberately limited to
`x86_64`; it refuses an AArch64 label because its SDKs are x86-64 targets:

- `wloc-service`: architecture-specific daemon plus `wloc-ctl`, procd init,
  UCI configuration, and network helpers;
- `luci-app-wificalling-location-gateway`: architecture-independent LuCI and
  rpcd files (`all` for IPK, `noarch` for APK).

OpenWrt 24.10 and iStoreOS 24.10 install IPK packages with `opkg`. OpenWrt
25.12 installs native APK v3 packages with `apk`; an IPK must never be renamed
to `.apk`.

## Reproducible package build

Build the static x86-64 runtime binaries used by the Docker matrix first:

```sh
./scripts/openwrt/build-x86_64-runtime.sh \
  --out-dir /absolute/path/dist/runtime/x86_64
```

The runtime build exports the musl toolchain from the immutable OpenWrt 24.10.8
SDK container. Dependency preparation is the only network-enabled phase;
compilation is locked, offline, and runs with read-only source. For AX6S,
continue to use `scripts/ci/verify-rust-openwrt.sh`, which targets AArch64
`cortex-a53`.

Then build the release packages:

```sh
./scripts/openwrt/build-release-packages.sh \
  --version 0.1.0 \
  --release 3 \
  --arch x86_64 \
  --service-bin /absolute/path/wloc-service \
  --ctl-bin /absolute/path/wloc-ctl \
  --out-dir /absolute/path/dist/openwrt-release
```

The builder uses immutable official OpenWrt SDK containers for 24.10.8 and
25.12.3. Product packaging runs with networking disabled and `--pull never`.
Images therefore must be fetched explicitly once. The output includes four
packages and `SHA256SUMS`.

## Three-platform Docker verification

The matrix is intentionally the same family of environments used by the
Wi-Fi Calling Gateway release process:

1. OpenWrt 24.10.8 x86-64 with `opkg`;
2. OpenWrt 25.12.3 x86-64 with `apk`;
3. iStoreOS 24.10.5 x86-64 with `opkg`.

Run:

```sh
./scripts/openwrt/verify-docker-matrix.sh \
  --dist-dir /absolute/path/dist/openwrt-release
```

For each environment the verifier boots `/sbin/init`, waits for ubus, installs
both package layers, enables and restarts the procd service, checks the Unix
control socket, and requires a valid `wloc.service/v1` status response. The
rootfs images do not contain the full dependency feeds used by a router, so
the isolated install test bypasses unresolved optional dependencies; it does
not claim that sing-box, nftables interception, DNS behavior, Wi-Fi Calling,
or an iPhone were exercised. Those remain AX6S/real-device gates.
