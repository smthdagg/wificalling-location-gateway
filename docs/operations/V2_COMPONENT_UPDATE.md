# V2 standalone component update and rollback

## Update contract

The root-only `/usr/sbin/wloc-component-update.sh` is the single update
boundary for the standalone WLOC package. LuCI may only reference an IPK
already staged under `/tmp/wloc-update/`; it cannot provide a URL or an
arbitrary filesystem path.

Before `opkg install` is called, the helper validates:

- regular local IPK and safe `control.tar.gz`/`data.tar.gz` members;
- standalone product identity, version, architecture, package format, and
  `wloc.service/v2` compatibility metadata;
- current router architecture, OpenWrt release family, package manager, and
  required kernel/module capabilities;
- a sidecar `PACKAGE.ipk.manifest` containing package/control/data SHA-256
  values and a detached `PACKAGE.ipk.sig` verified with the router's
  `/etc/wificalling-location-gateway/update.pub` `usign` key;
- free space for the package plus a bounded transaction reserve;
- current-version ordering, including explicit authorization for downgrade;
- a known-good rollback IPK.

The install is never remove-first. The helper takes a persistent transaction
snapshot of the WLOC configuration and the previous package, invokes the
package manager, restores configuration, restarts only the standalone
supervisor, and runs the bounded health command. Any install, restart, or
health failure restores the known-good package and configuration. A simulated
power loss or process interruption leaves the transaction marker; the LuCI
Recover action calls `recover` to complete the rollback.

The update path does not call `nft`, edit unrelated nftables rules, or disable
UDP 500/4500. The standalone supervisor owns the fail-open withdrawal/restart
boundary.

## State and resource policy

Persistent state is stored under
`/var/lib/wificalling-location-gateway/update` with a `0700` directory, `0600`
files, and a PID-bearing directory lock. A dead lock from a hard power cut is
reclaimed safely; a live or unreadable lock is not removed automatically.
`current.ipk`, `current.version`, and `status.json` retain the last known-good
update record. A second update is rejected while a transaction or live lock
exists. Preflight checks the smaller of the persistent-state and temporary
filesystems and reserves space for the package, rollback copy, commit copy,
and transaction overhead. On AX6S this prevents an update from starting when
flash or `/tmp` space is too low; no large duplicate root filesystem is
created.

Package metadata emitted by the builder will include:

```text
X-WLOC-Product: wificalling-location-gateway/v2
X-WLOC-Api: wloc.service/v2
X-WLOC-OpenWrt: 24.10+
```

`scripts/build-luci-ipk.sh` also writes the unsigned manifest sidecar. Release
automation must sign it before staging:

```sh
for package in dist/wificalling-location-gateway_*.ipk; do
  WLOC_UPDATE_SIGNING_KEY=/secure/release.sec \
    scripts/create-update-manifest.sh "$package"
done
```

Keep the resulting `.manifest` and `.sig` beside the IPK under
`/tmp/wloc-update/`; the update UI deliberately accepts only this local
staging workflow.

## LuCI behavior

The independent Component Update page exposes device/firmware/package
preflight, update phase, current/target versions, apply, and
interrupted-transaction recovery. Health and Monitor pages do not contain
update controls. The UI does not upload or fetch packages; staging remains an
explicit local router operation so the update source is observable and bounded.

An update status of `rollback_failed` is actionable and must not be presented
as a successful update. The operator should keep the router in passthrough,
restore the known-good IPK from local storage, and run `recover` again.

AX6S validation has confirmed the normal update and health-failure rollback
paths: 2.0.0-17 -> 2.0.0-18 commits only after WLOC, provider, and redirect
health are all present; a deliberately failing 2.0.0-19 activation restores
2.0.0-18 and removes the transaction directory. Hard power loss and flash-full
recovery remain separate hardware tests.
