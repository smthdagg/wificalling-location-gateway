# OpenWrt release packaging and Docker matrix

## Packaging boundary

The reference Wi-Fi Calling Gateway can publish one `all.ipk` and one
`noarch.apk` because its payload is Lua, JavaScript, and shell. WLOC adds two
Rust ELF executables, so the runtime package must name the real OpenWrt CPU
architecture. Marking an AArch64 or x86-64 ELF as `all` is invalid and can
install an unusable binary on another router.

The V2 staging baseline is `1.2.0`; the accepted V2.0 release will be
`2.0.0`. Both use one architecture-specific integrated package for each
package-manager generation. It combines the verified Wi-Fi Calling Gateway
1.7 payload, `wloc-service`, `wloc-ctl`, procd/UCI/network helpers, and the
unified LuCI/rpcd UI. The Docker builder is deliberately limited to `x86_64`;
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
  --version 1.2.0 \
  --release 1 \
  --arch x86_64 \
  --service-bin /absolute/path/wloc-service \
  --ctl-bin /absolute/path/wloc-ctl \
  --gateway-ipk /absolute/path/luci-app-wificalling-gateway_1.7.3-1_all.ipk \
  --gateway-sha256 VERIFIED_SHA256 \
  --out-dir /absolute/path/dist/openwrt-release
```

The builder uses immutable official OpenWrt SDK containers for 24.10.8 and
25.12.3. Product packaging runs with networking disabled and `--pull never`.
Images therefore must be fetched explicitly once. The output includes one IPK,
one native APK v3 package, and `SHA256SUMS`. The Gateway input is rejected
unless its package identity is `luci-app-wificalling-gateway`, its version is
1.7.x, its archive paths are safe, and its digest matches the explicit pin.

## Every release asset: four-environment Docker verification

The matrix installs every release package. It uses:

1. official OpenWrt 24.10.5 AArch64 rootfs with the AX6S/cortex-a53 IPK;
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

- `wificalling-location-gateway_1.2.0-r1_x86_64.ipk` for OpenWrt/iStoreOS 24.x;
- `wificalling-location-gateway-1.2.0-r1.apk` for OpenWrt 25.x.

After AX6S acceptance, rebuild these assets as `2.0.0-r1` before signing or
publishing the final release.

The current host-side plan/static package checks pass. The final Docker install
matrix and AX6S installation/upgrade/rollback result remain release gates and
must not be filled with a host-only result:

```text
Docker matrix: pending final release artifacts
AX6S migration/resource/rollback: pending real-device evidence
```
