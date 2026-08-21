# WLOC Location Service

Standalone Apple WLOC location service for OpenWrt / ImmortalWrt. This project
does not depend on, vendor, install, or manage Wi-Fi Calling Gateway.

[English guide](docs/WLOC_TUTORIAL_EN.md) · [中文指南](docs/WLOC_TUTORIAL_ZH.md) ·
[Development and test plan](DEVELOPMENT_TEST_PLAN.md) ·
[Security policy](SECURITY.md)

## Scope

The service provides:

- one standalone WLOC lifecycle supervisor and one root-only control socket;
- one independent UCI model at /etc/config/wloc-service;
- one profile per authorized LAN device;
- one explicit WLOC node reference per profile;
- Auto mode following that node exit, or Manual mode storing coordinates on the
  same profile;
- provider reuse through sing-box tiny/lite or an existing PassWall sing-box
  executable, without copying a second full binary;
- unified Overview, Basic Settings, Devices, Logs & Monitoring, Service Status,
  Component Update, and Help pages;
- bounded, redacted logs and transactional signed component updates.

Unknown protocol, invalid Geo data, provider failure, or service failure is
fail-open: the redirect is withdrawn and no default coordinate is invented.
Scope is limited to the assigned device, the two exact Apple WLOC hostnames,
and TCP 443. UDP 500/4500 is outside this project.

## AX6S installation

AX6S has limited overlay storage. Remove the old WLOC package before installing
the architecture-specific package, while preserving sing-box/PassWall and the
WLOC UCI/CA backups:

~~~sh
opkg list-installed | grep -E 'wificalling|wloc|sing-box|passwall'
cp -p /etc/config/wloc-service /tmp/wloc-service.backup 2>/dev/null || true
/etc/init.d/wloc-service stop 2>/dev/null || true
/etc/init.d/wificalling-location-gateway stop 2>/dev/null || true
opkg remove luci-app-wificalling-location-gateway wloc-service wloc-ctl 2>/dev/null || true
df -k /overlay /tmp
opkg install /tmp/wificalling-location-gateway_2.0.0-1_aarch64_cortex-a53.ipk
~~~

Do not use forced dependency removal and do not remove the selected provider.
The complete procedure is [AX6S deployment](docs/deployment/AX6S_DEPLOYMENT.md).
The current real-device record is [AX6S evidence](docs/testing/AX6S_REAL_DEVICE_2026-08-22.md);
the final package re-install evidence is recorded there. Real iPhone WLOC
traffic remains a separate open test because no client fixture was supplied.

The item-by-item audit is [V2 requirement audit](docs/testing/V2_REQUIREMENT_AUDIT.md).

## Build and verify

~~~sh
./scripts/ci/verify.sh

./scripts/openwrt/build-release-packages.sh \
  --version 2.0.0 --release 1 --arch x86_64 \
  --service-bin "$PWD/dist/runtime/x86_64/wloc-service" \
  --ctl-bin "$PWD/dist/runtime/x86_64/wloc-ctl" \
  --out-dir "$PWD/dist/openwrt-release"
~~~

The release builder accepts only this repository's WLOC sources. It produces
an architecture-matched IPK/APK package and checks the runtime architecture,
OpenWrt family, package format, free space, WLOC API metadata, and signed
manifest before an update.

## Layout

- src/: Rust WLOC protocol, TLS/H2 proxy, Geo, provider exit probing, and UDS API.
- openwrt/: procd, UCI, nftables/dnsmasq, LuCI, rpcd, and package files.
- scripts/: reproducible builds, package/update manifests, resource checks.
- tests/: Rust, shell, Python, and JavaScript contract tests.
- docs/: architecture, security, deployment, testing, and release records.

## Product boundary

The historical Wi-Fi Calling integration documents and adapter code are not
part of V2. ADR 0003 is authoritative:
[standalone product boundary](docs/adr/0003-standalone-wloc-product-boundary.md).
