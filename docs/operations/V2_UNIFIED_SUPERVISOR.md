# V2 standalone WLOC supervisor

## Ownership

`/etc/init.d/wificalling-location-gateway` is the only enabled v2 entry point.
Its single procd instance runs
`/usr/libexec/wificalling-location-gateway/unified-supervisor.sh`, which owns
the WLOC service, redirect lifecycle, and optional sing-box provider.

The legacy WLOC init script remains available for rollback but is disabled by
the migration/post-install step. No Wi-Fi Calling Gateway init script, UCI
file, package, or firewall table is part of the standalone steady state.

## Start ordering

1. Create a root-only volatile runtime directory and acquire the supervisor
   lock.
2. Stop any legacy WLOC instance and select/verify the configured sing-box
   provider in passthrough mode.
3. Start WLOC without its legacy redirect side effect and wait for its
   root-only Unix socket.
4. Wait up to the bounded startup timeout for both child processes and the
   WLOC socket before enabling the WLOC redirect.
5. Install only the WLOC-owned redirect/table and publish `intercepting`.

The supervisor never edits an external Gateway nftables table and never
intercepts UDP 500/4500.

## Failure and stop ordering

Any WLOC child exit, provider failure, health failure, or redirect setup failure
first calls `wloc-redirect-sync.sh stop`, then stops WLOC and returns to safe
passthrough. An explicit service stop, SIGTERM, or reload withdraws only
WLOC-owned rules. The WLOC table, policy route, DNS marker, socket, and
supervisor state are handled idempotently. A failed cleanup is reported as
`cleanup_unsafe`, never as a clean `stopped` state.

The WLOC daemon applies its persisted `enabled` state in supervised mode with
the first redirect installation deferred. The supervisor performs the actual
redirect install after readiness, so daemon state and firewall transition
cannot disagree during startup.

procd allows at most three supervisor restarts in one hour (`3600 5 3`). The
Rust supervisor policy additionally limits the managed child count to two
long-lived children plus one temporary probe and bounds restart backoff and
health polling.

## Rollback

```sh
/etc/init.d/wificalling-location-gateway stop
/etc/init.d/wificalling-location-gateway disable
/etc/init.d/wloc-service enable
/etc/init.d/wloc-service start
```

Rollback preserves `/etc/config/wloc-service`, the WLOC CA, and provider
references.
If unified startup fails, the supervisor leaves the router in passthrough and
does not delete persistent credentials or configuration.
