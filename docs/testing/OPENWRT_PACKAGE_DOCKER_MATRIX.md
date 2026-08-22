# OpenWrt release packaging and Docker matrix

## Packaging boundary

This is the integrated WiFi Calling Gateway + WLOC package. It does not depend
on the separate Gateway 1.7 repository. Its payload includes Rust ELF executables, so
the runtime package must name the real OpenWrt CPU architecture. Marking an
AArch64 or x86-64 ELF as `all` is invalid and can install an unusable binary on
another router.

The V2 release version is `2.0.0`. It uses one architecture-specific package
for each package-manager generation. It contains `wloc-service`, `wloc-ctl`,
Gateway/WLOC procd/UCI/network helpers, and the unified LuCI/rpcd UI. The
Docker builder is deliberately limited to `x86_64`;
it refuses an AArch64 label because these SDKs are x86-64 targets.

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
  --version 2.0.0 \
  --release 1 \
  --arch x86_64 \
  --service-bin /absolute/path/wloc-service \
  --ctl-bin /absolute/path/wloc-ctl \
  --ax6s-package /absolute/path/wificalling-location-gateway_2.0.0-1_aarch64_cortex-a53.ipk \
  --out-dir /absolute/path/dist/openwrt-release
```

The builder uses immutable official OpenWrt SDK containers for 24.10.8 and
25.12.3. Product packaging runs with networking disabled and `--pull never`.
Images therefore must be fetched explicitly once. The output includes the AX6S
AArch64 IPK, the x86_64 24.x IPK, the native APK v3 package, per-IPK update
manifests, and `SHA256SUMS`. The AX6S package is an architecture-correct
integrated package produced by the AX6S builder; it is validated for identity,
version, architecture, and integrated metadata before being included. The
builder accepts no Gateway IPK input. Package preflight validates the router
architecture, OpenWrt release family, package format, required kernel/module
capabilities, and available space.

## Every release asset: four-environment Docker verification

The matrix installs every release package. It uses:

1. official OpenWrt 24.10.5 AArch64 rootfs with the AX6S/cortex-a53 integrated IPK;
2. OpenWrt 24.10.8 x86-64 with the 24.x IPK;
3. OpenWrt 25.12.3 x86-64 with the native APK;
4. iStoreOS 24.10.5 x86-64 with the same 24.x IPK.

Run:

```sh
./scripts/openwrt/verify-docker-matrix.sh \
  --dist-dir /absolute/path/dist/openwrt-release
```

For each environment the verifier boots `/sbin/init`, waits for ubus, installs
the single integrated package, enables and restarts the procd service, checks the Unix
control socket, and requires a valid `wloc.service/v1` status response. The
verifier first validates every package against the release `SHA256SUMS`, binding
the Docker evidence to the exact files intended for upload. The
rootfs images do not contain the full dependency feeds used by a router, so
the isolated install test bypasses unresolved optional dependencies; it does
not claim that sing-box, nftables interception, DNS behavior, Wi-Fi Calling,
or an iPhone were exercised. Those remain AX6S/real-device gates. The V2
candidate has these target assets:

- `wificalling-location-gateway_2.0.0-1_aarch64_cortex-a53.ipk` for AX6S;
- `wificalling-location-gateway_2.0.0-r1_x86_64.ipk` for OpenWrt/iStoreOS 24.x;
- `wificalling-location-gateway-2.0.0-r1.apk` for OpenWrt 25.x.

The host-side release build, SHA-256 verification, and four-environment Docker
install matrix passed on 2026-08-22. Each case installed the single integrated Gateway/WLOC
package, enabled/restarted the service, created the control socket, and
returned a valid `wloc.service/v1` status:

```text
Redmi AX6S / OpenWrt 24.10.5|OpenWrt 24.10.5 aarch64_generic|installed|started|socket-ok|status-ok
OpenWrt 24.10.8|OpenWrt 24.10.8 x86_64|installed|started|socket-ok|status-ok
OpenWrt 25.12.3|OpenWrt 25.12.3 x86_64|installed|started|socket-ok|status-ok
iStoreOS 24.10.5|iStoreOS 24.10.5 x86_64|installed|started|socket-ok|status-ok
```

This matrix does not claim sing-box, nftables interception, DNS behavior,
Wi-Fi Calling, or real iPhone traffic. Those remain separate runtime/fixture
coverage items.
