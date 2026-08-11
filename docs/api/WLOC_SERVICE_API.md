# WLOC service control API v1

Status: frozen for standalone service development; reserved for a future LuCI
client. This document does not authorize or implement UI, WLOC response
patching, CA installation, traffic interception, or Gateway 1.7 integration.

## Transport boundary

- API identifier: `wloc.service/v1`.
- Local transport only: Unix socket `/var/run/wloc-service/control.sock`.
- The OpenWrt runtime must create the socket as root with mode `0600`.
- No TCP management listener is permitted.
- A future LuCI package must use an audited rpcd/ubus or root helper facade; it
  must not expose the socket directly to browsers.
- Maximum request frame: 16 KiB.
- Request deadline: 2 seconds.
- Maximum concurrent control connections: 2.
- Requests use strict JSON objects. Unknown top-level or `params` fields are
  rejected. A major API version mismatch returns `incompatible_version`.

## Request envelope

```json
{
  "api_version": "wloc.service/v1",
  "request_id": "req-1",
  "method": "status.get",
  "params": {}
}
```

`request_id` is 1–64 ASCII alphanumeric, dot, underscore, or hyphen bytes. It
is correlation metadata, not authentication, and must not contain credentials.

The only v1 methods are:

- `status.get`
- `control.enable`
- `control.disable`
- `control.reload`

Control authorization belongs to the root-only transport/facade. Adding a
method requires a new reviewed contract test; arbitrary debug or raw-dump
methods are prohibited.

## Response envelope

Exactly one of `result` or `error` is present:

```json
{
  "api_version": "wloc.service/v1",
  "request_id": "req-1",
  "result": {}
}
```

```json
{
  "api_version": "wloc.service/v1",
  "request_id": "req-1",
  "error": {
    "code": "invalid_config",
    "component": "service",
    "retryable": false
  }
}
```

Errors use stable enum codes. They contain no free-form provider payload,
request/response body, node credential, device IP/MAC, private key, or precise
real location.

## Reserved status model

The status snapshot is immutable and additive within v1:

```json
{
  "api_version": "wloc.service/v1",
  "generation": 1,
  "observed_at": 0,
  "desired_state": "disabled",
  "service_phase": "disabled",
  "safety": {
    "redirect_present": false,
    "watchdog_armed": false,
    "scope_valid": false,
    "ipv6_ready": false,
    "response_mode": "forward_original"
  },
  "engine": {
    "health": "stopped",
    "uptime_seconds": 0
  },
  "exit": {
    "state": "unknown",
    "checked_at": null
  },
  "geo": {
    "state": "unavailable",
    "expires_at": null
  },
  "assigned_device_configured": false,
  "last_error": null
}
```

`service_phase` values reserved by the domain model are `disabled`, `starting`,
`ready_passthrough`, `intercepting`, `degraded_passthrough`, and `draining`.
The UI must never translate these states into “device located”, “Wi-Fi Calling
activated”, or “emergency location verified”.

Device IP/MAC is not returned. If a future UI needs correlation, the runtime
may return a local, irreversible short identifier after separate review.
Coordinates may be exposed only to an authenticated local administrator by a
separately reviewed additive field; logs, support bundles, and default status
responses remain coordinate-free.

## Safety sequencing

Enable order is transactional:

1. validate configuration and exact scope;
2. start the engine in `forward_original` mode;
3. verify engine health;
4. arm an external watchdog;
5. verify the selected IPv6 policy;
6. install the exact redirect last.

Any failure compensates by withdrawing the redirect and stopping the engine.

Disable order is:

1. withdraw redirect first;
2. verify it is absent;
3. drain active work;
4. stop the engine.

Disable and redirect removal are idempotent. A daemon cannot be the sole owner
of watchdog cleanup because it cannot remove a redirect after it has died.

## Decision separation

Ingress and response decisions are independent:

- ingress: `bypass_mitm`, `route_to_mitm`, or `withdraw_redirect`;
- response: `forward_original` or, only after all protocol gates are closed,
  `patch_authorized`.

The current implementation always reports `forward_original`. Fresh Geo data
does not itself authorize response modification.
