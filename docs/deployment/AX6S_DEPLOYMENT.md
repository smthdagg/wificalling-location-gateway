# Redmi AX6S V2 deployment, migration and rollback

Target: Redmi AX6S (MediaTek MT7622, AArch64/cortex-a53) on compatible
OpenWrt/ImmortalWrt 24.x firmware.

## 1. Back up before replacing the old package

Create a LuCI system backup and separately preserve these conffiles:

```sh
cp -p /etc/config/wificalling-gateway /tmp/wificalling-gateway.backup
cp -p /etc/config/wloc-service /tmp/wloc-service.backup
sha256sum /etc/config/wificalling-gateway /etc/config/wloc-service
cp -p /etc/wloc-service/ca.pem /tmp/wloc-ca.pem.backup 2>/dev/null || true
cp -p /etc/wloc-service/ca.key /tmp/wloc-ca.key.backup 2>/dev/null || true

# Record the installed package set and available space before removing the
# old build. Keep these records off the release artifact.
opkg list-installed | grep -E 'wificalling|sing-box|passwall' > /tmp/wloc-old-packages.txt || true
df -k /overlay /tmp
```

Do not print or publish their contents: node links and credentials may be
sensitive. Keep the iPhone CA profile installed while upgrading unless the CA
is intentionally being rotated.

## 2. Remove the old build only after backup

The AX6S test device has insufficient persistent space for the old WLOC/Gateway
package and the new integrated package to coexist. Stop and disable the old
services, then remove only the old application packages. Do not remove
`sing-box` if it is the selected shared tiny/PassWall runtime, and do not use
`opkg remove --force-removal-of-dependent-packages`.

```sh
/etc/init.d/wloc-service stop 2>/dev/null || true
/etc/init.d/wificalling-gateway stop 2>/dev/null || true
/etc/init.d/wloc-service disable 2>/dev/null || true
/etc/init.d/wificalling-gateway disable 2>/dev/null || true
opkg remove luci-app-wificalling-location-gateway wloc-service wloc-ctl 2>/dev/null || true
df -k /overlay /tmp
```

If the old release used a different package name, remove that exact package
from `/tmp/wloc-old-packages.txt`; keep the config backups and CA files.

## 3. Install the integrated V2 package

Use the architecture-specific asset and verify it against the release
`SHA256SUMS`:

```sh
opkg install /tmp/wificalling-location-gateway_1.2.0-1_aarch64_cortex-a53.ipk
```

The integrated package replaces/provides both application components and
declares both UCI files as conffiles. Because the AX6S test device is space
constrained, the preceding removal step is intentional; restore the backups
only if the package manager reports a conffile conflict.

## 4. Verify migration and startup

```sh
opkg status wificalling-location-gateway
/etc/init.d/wificalling-gateway status
/etc/init.d/wloc-service status
test -S /var/run/wloc-service/control.sock
/usr/sbin/wloc-ctl status
sha256sum /etc/config/wificalling-gateway /etc/config/wloc-service
logread -e wloc-service
/usr/libexec/wificalling-location-gateway/singbox-runtime.sh path
/usr/libexec/wificalling-location-gateway/singbox-runtime.sh version
```

The package status must report version `1.2.0-1`, the unified supervisor init
service must be available, the Unix socket must exist, and control output must
identify `wloc.service/v1`. Profile CRUD uses the additive `wloc.service/v2`
surface and persists through UCI; LuCI Apply restarts the unified supervisor.
If a configuration hash changes, inspect only the local UCI diff and restore the
corresponding backup before continuing.

The runtime provider must resolve to the AX6S-tested sing-box tiny/lite binary
or the existing PassWall sing-box binary. V2 reuses that executable and does
not install a duplicate full-size copy; see
[`V2_SINGBOX_RUNTIME.md`](../operations/V2_SINGBOX_RUNTIME.md).

## 5. LuCI and iPhone validation

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

## 6. Safety checks

- Normal HTTPS from other LAN devices must not be signed by the WLOC CA.
- The WLOC nftables table must be independent of the Gateway table.
- UDP 500/4500 must never enter the WLOC redirect.
- Invalid Geo, protocol, TLS, or service state must not create a default
  coordinate; traffic must pass through unchanged or the redirect must be
  withdrawn.
- CA private keys, node credentials, device identifiers, raw WLOC payloads, and
  precise personal locations must never be copied into logs or support bundles.

## 7. Rollback

Install the previous architecture-matching package directly, then restore only
the affected UCI backup if required. Do not delete `/etc/config` or regenerate
the CA during ordinary rollback. If the service cannot be restored, disable
WLOC interception first; Wi-Fi Calling Gateway operation must remain isolated.

## 8. V2 resource and release evidence

Before calling the AX6S run accepted, fill in
[`AX6S_RESOURCE_EVIDENCE.template.md`](../testing/AX6S_RESOURCE_EVIDENCE.template.md)
with coarse values only. The run must cover disabled, one-profile,
multi-profile, degraded, restart, low-space, successful update, interrupted
update and rollback. Do not publish serials, addresses, node names, raw logs,
credentials, CA material or precise locations. Host-side resource and package
contracts do not replace real AX6S measurements.

## Build evidence

V2 uses the pinned OpenWrt 24.10.8 mt7622 cross toolchain and Rust
1.90. The release binaries are static AArch64 ELF files. The integrated package
build validates the Gateway input identity/version/digest and preserves both
configuration paths. Construction evidence is recorded in
[`STANDALONE_AX6S_PACKAGE.tdd.md`](../testing/STANDALONE_AX6S_PACKAGE.tdd.md).
