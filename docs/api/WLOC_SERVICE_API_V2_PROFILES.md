# WLOC service v2 profile slice

This document records the V2-01/V2-02 profile-management slice. It is additive to
the existing `wloc.service/v1` API; no v1 request is interpreted as a profile
request.

## Bounded profile model

One `device` profile is the unit of configuration for a LAN device. The model
contains:

- a stable lower-case `id`;
- a display `label`;
- an assigned private LAN IPv4 address for explicit v2 device sections;
- a fixed node reference;
- a WLOC source (`auto` or `manual`), optional manual coordinates, and an
  optional manual location reference;
- an `enabled` flag.

The model is deliberately bounded for small OpenWrt gateways:

- at most 8 profiles;
- profile IDs up to 32 bytes;
- labels up to 48 bytes;
- node references up to 96 bytes;
- manual location references up to 64 bytes;
- serialized profile configuration up to 8 KiB;
- redacted status output up to 4 KiB.
- complete UCI input up to 32 KiB before section accumulation.

Validation is performed before a model replacement. A failed replacement
leaves the previous model unchanged.

## Legacy migration

If no `config device` sections exist, the v1 singleton fields are projected
into a deterministic profile with ID `default`. The projection preserves the
assigned device, node reference, enabled state, WLOC source, and manual
coordinates. Repeating the projection is idempotent.

When explicit device sections exist, they take precedence over the legacy
singleton fields. Invalid IDs, addresses, node modes, location pairs, or
duplicate IDs reject the UCI parse before any runtime operation.

The current daemon consumes exactly one explicit profile. If more than one
profile is configured before the unified multi-device runtime lands, it stays
disabled rather than selecting a profile implicitly. Explicit bindings are
private LAN IPv4 addresses only: unspecified, loopback, multicast, broadcast,
and IPv4 link-local values are rejected. MAC and IPv6 bindings are rejected at
model validation because the current OpenWrt TPROXY and source-device router
are IPv4-scoped; they are not accepted and then silently left inactive. The
legacy singleton projection may still have no address so an existing
installation can continue its migration behavior. The sing-box probe requires
a matching WLOC provider node; it never falls back to an unrelated outbound.

A missing UCI file retains the v1 unconfigured-default behavior. An existing
but malformed or oversized UCI file is fail-closed and cannot be re-enabled by
the v1 control socket until the file is corrected.

## v2 request envelope

Profile methods use the existing control-frame bound and the version string
`wloc.service/v2`:

```json
{
  "api_version": "wloc.service/v2",
  "request_id": "req-1",
  "method": "profile.get",
  "params": { "profile_id": "phone" }
}
```

The V2-01 decoder accepts `profile.list`, `profile.get`, `profile.create`,
`profile.update`, and `profile.delete`. Unknown top-level or parameter fields,
invalid profile IDs, invalid modes, invalid addresses, and out-of-range
coordinates fail before dispatch. `get`, `update`, and `delete` require a
profile ID. `create` requires the complete profile identity, label, assigned
device, node, node mode, location source, and enabled fields; manual creates
also require both coordinates.

Profile status is redacted: it reports whether an assigned device or manual
location is configured, but never returns the device address, node reference,
coordinates, credentials, or private key material.

The production control-plane dispatcher routes these methods through a bounded
UCI-backed adapter. It validates the candidate model before issuing separate
`uci` arguments, commits once, and reverts staged changes on failure; it never
interpolates a shell command or returns node credentials, device addresses, or
coordinates. The active runtime still follows the unified supervisor boundary:
LuCI's Apply action restarts that boundary after UCI changes, while direct
`wloc-ctl` profile mutations take effect after the next unified-service restart.
A server created through the legacy constructor returns an `unavailable` v2
error for profile operations until an adapter is supplied.

`wloc-ctl` exposes `profile-list`, `profile-get`, `profile-create`,
`profile-update`, and `profile-delete`; the existing v1 commands and envelope
remain unchanged. OpenWrt UCI writes, runtime redirect lifecycle, and unified
supervisor wiring remain outside this control-plane slice.
