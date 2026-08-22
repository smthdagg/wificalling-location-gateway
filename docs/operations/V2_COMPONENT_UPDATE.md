# V2 component update and rollback

## Update contract

The root-only `/usr/sbin/wloc-component-update.sh` is the single update
boundary for the unified Gateway/WLOC package. LuCI may only reference an IPK
already staged under `/tmp/wloc-update/`; it cannot provide a URL or an
arbitrary filesystem path.

Before `opkg install` is called, the helper validates:

- regular local IPK and safe `control.tar.gz`/`data.tar.gz` members;
- unified package identity, version, architecture, Gateway `1.7`, and
  `wloc.service/v2` compatibility metadata;
- a sidecar `PACKAGE.ipk.manifest` containing package/control/data SHA-256
  values and a detached `PACKAGE.ipk.sig` verified with the router's
  `/etc/wificalling-location-gateway/update.pub` `usign` key;
- free space for the package plus a bounded transaction reserve;
- current-version ordering, including explicit authorization for downgrade;
- a known-good rollback IPK.

The install is never remove-first. The helper takes a persistent transaction
snapshot of the two component configuration files and the previous package,
invokes the package manager, restores configuration, restarts only the unified
supervisor, and runs the bounded health command. Any install, restart, or
health failure restores the known-good package and configuration. A simulated
power loss or process interruption leaves the transaction marker; the LuCI
Recover action calls `recover` to complete the rollback.

The update path does not call `nft`, edit nftables rules, disable UDP 500/4500,
or stop the stable Gateway data-plane owner directly. The supervisor owns the
existing fail-open withdrawal/restart boundary.

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

Package metadata emitted by the builder includes:

```text
X-WFC-Product: wificalling-location-gateway/v2
X-WFC-Gateway: 1.7
X-WFC-Wloc-Api: wloc.service/v2
```

`scripts/build-luci-ipk.sh` also writes the unsigned manifest sidecar. Release
automation must sign it before staging:

```sh
for package in dist/wificalling-location-gateway_*.ipk; do
  WFC_UPDATE_SIGNING_KEY=/secure/release.sec \
    scripts/create-update-manifest.sh "$package"
done
```

Keep the resulting `.manifest` and `.sig` beside the IPK under
`/tmp/wloc-update/`; the update UI deliberately accepts only this local
staging workflow.

## LuCI behavior

The Health and Monitor page exposes update phase, current/target versions,
reason code, package preflight, apply, and interrupted-transaction recovery.
The UI does not upload or fetch packages; staging remains an explicit local
router operation so the update source is observable and bounded.

An update status of `rollback_failed` is actionable and must not be presented
as a successful update. The operator should keep the router in passthrough,
restore the known-good IPK from local storage, and run `recover` again.
