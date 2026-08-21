# WLOC service v2 profile slice

This document records the V2-01 profile-management slice. It is additive to
the existing `wloc.service/v1` API; no v1 request is interpreted as a profile
request.

## Bounded profile model

One `device` profile is the unit of configuration for a LAN device. The model
contains:

- a stable lower-case `id`;
- a display `label`;
- an assigned IPv4/IPv6 address or six-octet MAC address for explicit v2
  device sections;
- a node reference and `node_mode` (`fixed` or `gateway_default`);
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
profile ID.

Profile status is redacted: it reports whether an assigned device or manual
location is configured, but never returns the device address, node reference,
coordinates, credentials, or private key material.

Runtime dispatch, multi-device nftables routing, the unified procd supervisor,
LuCI pages, and component updates are subsequent v2 issues.
