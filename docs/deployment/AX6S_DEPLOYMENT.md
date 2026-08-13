# Redmi AX6S deployment and migration

Target: Redmi AX6S (MediaTek MT7622, AArch64/cortex-a53) on compatible
OpenWrt/ImmortalWrt 24.x firmware.

## 1. Back up before installation

Create a LuCI system backup and separately preserve these conffiles:

```sh
cp -p /etc/config/wificalling-gateway /tmp/wificalling-gateway.backup
cp -p /etc/config/wloc-service /tmp/wloc-service.backup
sha256sum /etc/config/wificalling-gateway /etc/config/wloc-service
```

Do not print or publish their contents: node links and credentials may be
sensitive. Keep the iPhone CA profile installed while upgrading unless the CA
is intentionally being rotated.

## 2. Install the integrated 1.0 package

Use the architecture-specific asset and verify it against the release
`SHA256SUMS`:

```sh
opkg install /tmp/wificalling-location-gateway_1.0.0-1_aarch64_cortex-a53.ipk
```

Do **not** uninstall the old Gateway or WLOC packages first. The integrated
package replaces/provides both application components and declares both UCI
files as conffiles, so direct installation repairs missing files while
preserving configuration.

## 3. Verify migration and startup

```sh
opkg status wificalling-location-gateway
/etc/init.d/wificalling-gateway status
/etc/init.d/wloc-service status
test -S /var/run/wloc-service/control.sock
/usr/sbin/wloc-ctl status
sha256sum /etc/config/wificalling-gateway /etc/config/wloc-service
logread -e wloc-service
```

The package status must report version `1.0.0-1`, both init services must be
available, the Unix socket must exist, and control output must identify
`wloc.service/v1`. If a configuration hash changes, inspect only the local UCI
diff and restore the corresponding backup before continuing.

## 4. LuCI and iPhone validation

1. Open **Services → WifiCalling&Wloc Gateway**.
2. Confirm prior nodes and device policies remain present.
3. Confirm Wi-Fi Calling monitoring still observes the assigned device without
   any WLOC rule touching UDP 500/4500.
4. On **WLOC Settings**, verify the selected follow device and saved locations.
5. Switch **Auto (follow node)** to **Manual location**, apply a known saved
   location, then switch back. No control-socket refusal or configuration loss
   is acceptable.
6. Confirm the iPhone profile fingerprint matches LuCI and full trust remains
   enabled for `wloc-service root CA`.
7. With phone VPN/WARP disabled, trigger Maps/Weather and confirm the updated
   target in **WLOC Monitor & Log**.
8. Place a real incoming and outgoing Wi-Fi call. `ASSURED` is network evidence,
   not proof of carrier activation.

## 5. Safety checks

- Normal HTTPS from other LAN devices must not be signed by the WLOC CA.
- The WLOC nftables table must be independent of the Gateway table.
- UDP 500/4500 must never enter the WLOC redirect.
- Invalid Geo, protocol, TLS, or service state must not create a default
  coordinate; traffic must pass through unchanged or the redirect must be
  withdrawn.
- CA private keys, node credentials, device identifiers, raw WLOC payloads, and
  precise personal locations must never be copied into logs or support bundles.

## 6. Rollback

Install the previous architecture-matching package directly, then restore only
the affected UCI backup if required. Do not delete `/etc/config` or regenerate
the CA during ordinary rollback. If the service cannot be restored, disable
WLOC interception first; Wi-Fi Calling Gateway operation must remain isolated.

## Build evidence

Release 1.0 uses the pinned OpenWrt 24.10.8 mt7622 cross toolchain and Rust
1.90. The release binaries are static AArch64 ELF files. The integrated package
build validates the Gateway input identity/version/digest and preserves both
configuration paths. Construction evidence is recorded in
[`STANDALONE_AX6S_PACKAGE.tdd.md`](../testing/STANDALONE_AX6S_PACKAGE.tdd.md).
