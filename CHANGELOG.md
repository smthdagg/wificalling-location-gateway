# Changelog

All notable changes are documented here. Versions follow Semantic Versioning.

## [Unreleased]

### Added

- `star-history-chart` workflow regenerates `docs/images/star-history.svg` daily
  from the official GitHub stargazer API using the auto-injected Actions token
  (no third-party service, no token in the repository); README embeds the
  committed chart instead of the star-history.com embed.

## [1.0.2] - 2026-08-15

Router package update (v1.0.2-2) fixing node-switch follow-up and adding a
manual refresh for the WLOC monitor.

### Added

- **Manual refresh IP button** on the WLOC Monitor page, next to the followed
  device: it discards cached probe evidence, re-probes the followed node
  immediately, and rewrites the status file so the exit IP updates on the spot
  (new `control.refresh` control command wired through `wloc-ctl` and the
  `luci.wloc` rpcd bridge; a failed probe shows the reason without restarting
  the daemon).
- **Connected-device picker in the "Add LAN device" form**: the edit modal
  lists the LAN devices detected from the DHCP leases and the ARP cache
  (hostname + real IP, router and already-bound IPs excluded); picking one
  fills in the device name and the LAN IP field with the device's actual
  address. The IP placeholder now shows the router's real subnet pattern
  instead of a hardcoded 192.168.31.x.
- The monitor page ships under a versioned LuCI view name like the settings
  page, so an updated page is never served from the browser cache.

### Fixed

- **Node switches are now followed within seconds.** Previously the service
  only re-probed on its 600s housekeeping tick and the config fingerprint
  covered only the running `sing-box.json`, which the Gateway does not
  necessarily rewrite on a binding change - the monitor kept showing the old
  exit IP for up to ten minutes.
  - The fingerprint now also covers the device-policy UCI file
    (`/etc/config/wificalling-gateway`), so any binding change triggers an
    immediate re-probe.
  - The probe selects the node from the UCI device policy first (the user's
    source of truth) instead of trusting possibly stale sing-box route rules.
  - Housekeeping runs every 10 seconds; the probe itself still only runs when
    the fingerprint changed or cached evidence is stale.
- **The router LAN address is no longer hardcoded to 192.168.31.1.** The CA
  profile URL, the DNS hijack, and the matching TPROXY rule now derive the
  router IP from `uci network.lan.ipaddr` (falling back to the `br-lan`
  address), and the LuCI pages build the profile link from the address the
  admin is actually using - so certificate installation and WLOC interception
  work on any LAN subnet, not only 192.168.31.x. The FAQ profile link is now
  a tappable link instead of static text. The packaged export script and the
  daemon's upstream-IP filter use the runtime LAN address as well.
- **The shipped default config no longer pins an example device IP.** A fresh
  install no longer ships `assigned_device 192.168.31.X`; when no follow
  device is chosen in LuCI, the daemon follows the first device policy of the
  Gateway config, so WLOC works out of the box on any subnet.
- The packaged UI no longer lags the repository copy: probe failure reasons
  and the newer translation table were missing from earlier release packages.

## [1.0.1] - 2026-08-14

### Fixed

- WLOC monitor now follows the followed device's node immediately when the
  node is switched in the Wi-Fi Calling settings (probe-config fingerprint
  detection instead of waiting for the 300s cache).
- The monitor shows the probe failure reason (node DNS resolution failed /
  connection timed out / unreachable) instead of a bare empty exit IP.
- The exit probe no longer deadlocks on bad nodes whose sing-box output
  never ends (kill the probe child before draining its stderr).

## [1.0.0] - 2026-08-13

### Added

- One integrated, project-named package containing Wi-Fi Calling Gateway 1.7,
  the Rust WLOC service and control client, and unified LuCI/rpcd management.
- AX6S AArch64/cortex-a53 IPK plus x86-64 IPK for OpenWrt/iStoreOS 24.x and
  native APK v3 for OpenWrt 25.x.
- Automatic node-following and manual location modes, CA/profile lifecycle,
  bounded TLS-over-HTTP/2 handling, exit/Geo resolution, and status logging.
- Reproducible pinned builds, SHA-256 release manifests, dependency/license
  auditing, secret scanning, coverage gates, and four-environment Docker smoke
  verification covering every release asset.

### Fixed

- Automatic-to-manual WLOC mode switching now uses the live control socket and
  returns a controlled error without losing the saved configuration.
- Reinstall and upgrade preserve both `/etc/config/wificalling-gateway` and
  `/etc/config/wloc-service`; users no longer need to remove either component.

### Safety

- WLOC interception remains isolated from UDP 500/4500 and the Gateway table.
- Invalid Geo/protocol/TLS state never produces a default fake coordinate.

[1.0.2]: https://github.com/smthdagg/wificalling-location-gateway/releases/tag/v1.0.2
[1.0.1]: https://github.com/smthdagg/wificalling-location-gateway/releases/tag/v1.0.1
[1.0.0]: https://github.com/smthdagg/wificalling-location-gateway/releases/tag/v1.0.0
