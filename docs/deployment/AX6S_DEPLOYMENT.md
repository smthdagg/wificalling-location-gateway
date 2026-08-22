# Redmi AX6S V2 deployment, migration and rollback

Target: Redmi AX6S (MediaTek MT7622, AArch64/cortex-a53) on compatible
OpenWrt/ImmortalWrt 24.10 firmware.

## 1. Record and back up the WLOC state

The current product is the integrated WiFi Calling Gateway + WLOC project. It
is independent from the separate Gateway 1.7 repository, but this package
contains and manages both in-repository modules and both UCI states.

~~~sh
opkg list-installed | grep -E 'wificalling|wloc|sing-box|passwall' > /tmp/wloc-old-packages.txt || true
cp -p /etc/config/wloc-service /tmp/wloc-service.backup 2>/dev/null || true
cp -p /etc/config/wificalling-gateway /tmp/wificalling-gateway.backup 2>/dev/null || true
cp -p /etc/wloc-service/ca.pem /tmp/wloc-ca.pem.backup 2>/dev/null || true
cp -p /etc/wloc-service/ca.key /tmp/wloc-ca.key.backup 2>/dev/null || true
sha256sum /etc/config/wloc-service 2>/dev/null || true
df -k /overlay /tmp
~~~

Keep provider packages and binaries installed. Never publish UCI contents,
node credentials, CA keys, device identifiers, or precise test locations.

## 2. Remove the old WLOC package before installing V2

AX6S overlay storage cannot safely hold the old and new WLOC packages at the
same time. Stop and disable only the old WLOC ownership, then recheck space.
Do not remove sing-box tiny/lite or a PassWall-provided sing-box executable.

~~~sh
/etc/init.d/wificalling-location-gateway stop 2>/dev/null || true
/etc/init.d/wificalling-gateway stop 2>/dev/null || true
/etc/init.d/wloc-service stop 2>/dev/null || true
/etc/init.d/wloc-service disable 2>/dev/null || true
/etc/init.d/wificalling-gateway disable 2>/dev/null || true
opkg remove luci-app-wificalling-location-gateway wificalling-location-gateway wloc-service wloc-ctl 2>/dev/null || true
df -k /overlay /tmp
~~~

Do not use: opkg remove --force-removal-of-dependent-packages. If the old
package has another exact name, remove only that WLOC package listed in
/tmp/wloc-old-packages.txt; preserve the provider.

## 3. Install the architecture-matched package

Verify the release checksum before installing:

~~~sh
sha256sum -c /tmp/SHA256SUMS --ignore-missing
opkg install /tmp/wificalling-location-gateway_2.0.0-1_aarch64_cortex-a53.ipk
~~~

The package owns /etc/config/wloc-service and
/etc/config/wificalling-gateway as conffiles. It declares the integrated
product metadata and carries the Gateway and WLOC runtime/UI payloads; it has
no dependency on the separate Gateway 1.7 repository.

A direct `opkg` migration deliberately invalidates any older transactional
rollback IPK whose version does not match the newly installed package. This
prevents a WLOC-only package from becoming the rollback target of the
integrated product. After verifying the installed package, stage that exact
known-good IPK and its signed manifest for future component updates, then
initialize the updater record before using LuCI Component Update:

~~~sh
mkdir -p /var/lib/wificalling-location-gateway/update
cp -p /tmp/wificalling-location-gateway_2.0.0-1_aarch64_cortex-a53.ipk \
  /var/lib/wificalling-location-gateway/update/current.ipk
chmod 0600 /var/lib/wificalling-location-gateway/update/current.ipk
printf '%s\n' 2.0.0-1 > /var/lib/wificalling-location-gateway/update/current.version
~~~

Use the actual installed version and verified package filename; do not copy an
older WLOC-only artifact into this baseline.

## 4. Verify the integrated lifecycle

~~~sh
opkg status wificalling-location-gateway
/etc/init.d/wificalling-location-gateway status
/etc/init.d/wloc-service status
test -S /var/run/wloc-service/control.sock
/etc/init.d/wificalling-gateway status
/usr/sbin/wloc-ctl status
/usr/libexec/wificalling-location-gateway/singbox-runtime.sh path
/usr/libexec/wificalling-location-gateway/singbox-runtime.sh version
/usr/sbin/wloc-health.sh
logread -e wloc-service
~~~

The provider path must resolve to the AX6S-tested sing-box tiny/lite binary or
the retained PassWall sing-box binary. WLOC must use one provider executable
and must not install a duplicate full-size copy.

## 5. LuCI and authorized-device validation

1. Open Services → WiFi Calling + WLOC Gateway.
2. Confirm Overview, Basic Settings, Devices, Logs & Monitoring,
   Service Status, Component Update, and Help are present.
3. Create one device profile with its private LAN address and explicit WLOC
   node reference.
4. Test Auto follow selected node, then Manual location. Manual coordinates
   must be written to that same device profile.
5. Confirm the status page reports WLOC, provider, redirect, and profile state.
6. Install and trust the local WLOC CA only on the authorized test iPhone.
7. With other VPNs disabled, trigger Maps/Weather and confirm the WLOC event
   log and effective location. Normal HTTPS and other LAN devices must pass
   through unchanged.

## 6. Safety and resource checks

- Only the assigned device, the two exact Apple WLOC hostnames, and TCP 443
  are in scope.
- UDP 500/4500 and all external nftables tables are out of scope.
- Invalid protocol, Geo, provider, or service state withdraws the redirect and
  never invents a coordinate.
- Logs and support bundles stay bounded and redact credentials, device
  identifiers, raw WLOC data, and precise locations.
- Record free overlay space, RAM, process RSS, threads, package size, and the
  provider binary size in the evidence template.

## 7. Rollback

Stop WLOC interception first, then install the previous architecture-matched
WLOC package and restore only /etc/config/wloc-service if needed. Keep the
provider and CA unless deliberately rotating them. If rollback health fails,
leave WLOC disabled and preserve the transaction bundle for recovery.

See docs/testing/STANDALONE_AX6S_PACKAGE.tdd.md and
docs/testing/AX6S_RESOURCE_EVIDENCE.template.md.
