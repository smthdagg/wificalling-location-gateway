# Unified WLOC control API v2 draft

Status: proposed contract; profile request decoding and bounded local runtime
primitives are implemented, and production source-device multi-profile
Geo/patch routing is now integrated. Live v2 profile mutation dispatch remains
a subsequent control-plane slice. It does not change the frozen
`wloc.service/v1` contract.

## Purpose

v2 replaces the single global WLOC control model with one standalone WLOC
control plane. It coordinates device profiles, node bindings, WLOC auto/manual
location, health, logs, diagnostics, and component updates through one
root-only local Unix socket or an audited rpcd facade.

The API is an administrative control plane. It is not exposed to LAN TCP
clients and it never accepts credentials, raw WLOC bodies, private keys, or
unbounded diagnostic payloads.

## Transport limits

- API identifier: `wloc.service/v2`.
- Unix socket only; no TCP management listener.
- Socket directory mode `0700`; socket mode `0600`.
- Maximum frame: 16 KiB for control requests and normal responses.
- Maximum concurrent control operations: 2.
- One update job and one diagnostic bundle job at a time.
- Log queries are paginated; a request may return at most 100 events.
- Every request uses a bounded deadline and a validated request id.
- Unknown fields and unknown methods are rejected.

## Envelope

```json
{
  "api_version": "wloc.service/v2",
  "request_id": "req-1",
  "method": "profile.status.get",
  "params": {
    "profile_id": "device-1"
  }
}
```

Responses contain exactly one `result` or `error`. Errors use stable codes and
contain no provider payload, node credential, private key, raw protocol data,
or unbounded command output.

## Profile methods

| Method | Purpose | Side effect |
|---|---|---|
| `profile.list` | List profiles and compact health summaries | no |
| `profile.get` | Read one profile's effective configuration and status | no |
| `profile.create` | Create a validated profile | configuration |
| `profile.update` | Update profile fields transactionally | configuration |
| `profile.delete` | Disable, clean redirect, then remove profile | configuration |
| `profile.enable` | Enable WLOC for one profile | runtime |
| `profile.disable` | Withdraw redirect and disable one profile | runtime |
| `profile.reload` | Apply profile/config changes | runtime |
| `profile.node_test` | Run a bounded node test | background job |
| `profile.refresh` | Force bounded exit/Geo refresh | background job |
| `profile.wloc.set_mode` | Set `auto` or `manual` mode | runtime/config |
| `profile.wloc.set_location` | Set validated manual coordinates/preset | runtime/config |
| `profile.wloc.clear_location` | Return profile to auto mode | runtime/config |

`profile_id` is a bounded local identifier (`[a-z0-9_]{1,32}`) validated at
the API boundary. A profile may expose its
administrator-visible IP/MAC through the authenticated local LuCI facade, but
the default wire status must not leak device material to arbitrary callers.

## Status methods

| Method | Purpose |
|---|---|
| `system.status.get` | Unified WLOC service summary |
| `profile.status.get` | One profile's state and reason |
| `profile.status.list` | Paginated status for all profiles |
| `diagnostics.health.get` | Detailed bounded health checks |
| `diagnostics.self_test` | Run a bounded self-test sequence |
| `diagnostics.bundle.create` | Create a redacted, size-limited support bundle |

Profile status must distinguish at least:

- disabled;
- preparing;
- passthrough;
- intercepting;
- degraded passthrough;
- draining;
- migration required;
- update required.

Each degraded or failed state carries a stable `reason_code`, `component`,
`retryable`, and `observed_at` value. A green status is allowed only when the
corresponding process, node, Geo evidence, proxy, IPv4/IPv6 policy, and
redirect state have been observed.

## Log methods

| Method | Purpose |
|---|---|
| `log.query` | Paginated structured event query |
| `log.clear` | Clear selected profile or global ring buffer |
| `log.debug.enable` | Enable time-limited bounded debug mode |
| `log.debug.disable` | Disable debug mode and remove samples |

Each event uses a common envelope:

```json
{
  "time": 0,
  "level": "warn",
  "component": "wloc",
  "profile_id": "device-1",
  "event": "geo_probe_failed",
  "outcome": "unavailable",
  "reason_code": "timeout",
  "retryable": true,
  "redaction_version": 1
}
```

The shipped runtime keeps the WLOC structured event file under 64 KiB and
rejects individual events over 2 KiB. WLOC activity logs also enforce a
64 KiB default byte cap after the per-device record cap; deployments may lower
that cap through the local OpenWrt runtime environment.

The API never returns raw WLOC request/response bodies, credentials, tokens,
private keys, or arbitrary process output. Debug mode may expose bounded
metadata such as byte counts, status code, duration, and body hash only.

## Component update methods

| Method | Purpose |
|---|---|
| `component.list` | Current versions and compatibility state |
| `component.check` | Check a trusted update manifest |
| `component.update.prepare` | Validate architecture, space, memory, and package |
| `component.update.apply` | Apply one verified update transaction |
| `component.update.status` | Read update progress and result |
| `component.rollback` | Restore the last verified package/config state |

Updates must use a trusted manifest, verify architecture and SHA-256/signature,
stage at most one package and one rollback copy, preserve UCI/CA data, and
return to a healthy passthrough state before committing the new runtime.

## Compatibility

The v1 API remains available through a compatibility facade during one release
cycle. v1 methods map only to the explicitly selected default profile; they may
not silently operate on multiple profiles. The facade is removed only after the
v2 migration and rollback tests are complete.
