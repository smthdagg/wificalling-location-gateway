# V2 unified Gateway/WLOC supervisor

## Ownership

`/etc/init.d/wificalling-location-gateway` is the only enabled v2 entry point.
Its single procd instance runs
`/usr/libexec/wificalling-location-gateway/unified-supervisor.sh`, which
coordinates the existing Gateway and WLOC children during the one-release
migration window.

The legacy `wificalling-gateway` and `wloc-service` init scripts remain
available for rollback but are disabled by the migration/post-install step.
They are not independently enabled in the v2 steady state.

## Start ordering

1. Create a root-only volatile runtime directory and acquire the supervisor
   lock.
2. Stop any legacy child instances and start the Gateway in passthrough.
3. Start WLOC and wait for its root-only Unix socket.
4. Check both child processes before enabling the WLOC redirect.
5. Install only the WLOC-owned redirect/table and publish `intercepting`.

The supervisor never edits the stable Gateway nftables table and never
intercepts UDP 500/4500.

## Failure and stop ordering

Any child exit, health failure, redirect setup failure, SIGTERM, or reload
first calls `wloc-redirect-sync.sh stop`, then stops WLOC and Gateway. The
WLOC table, policy route, DNS marker, socket, and supervisor state are removed
idempotently. The stable Gateway can remain available in passthrough during a
WLOC failure.

procd allows at most three supervisor restarts in one hour (`3600 5 3`). The
Rust supervisor policy additionally limits the managed child count to two
long-lived children plus one temporary probe and bounds restart backoff and
health polling.

## Rollback

```sh
/etc/init.d/wificalling-location-gateway stop
/etc/init.d/wificalling-location-gateway disable
/etc/init.d/wloc-service enable
/etc/init.d/wificalling-gateway enable
/etc/init.d/wificalling-gateway start
/etc/init.d/wloc-service start
```

Rollback preserves `/etc/config/*`, the WLOC CA, and Gateway node material.
If unified startup fails, the supervisor leaves the router in passthrough and
does not delete persistent credentials or configuration.
