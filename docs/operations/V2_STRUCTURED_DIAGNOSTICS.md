# V2 structured diagnostics

## Event contract

Standalone WLOC lifecycle, device-profile, provider, and interception events
use one JSONL envelope:

```json
{
  "timestamp": 1710000000,
  "component": "wloc|provider|redirect|device",
  "profile_scope": "service|device-policy",
  "severity": "info|warning|error",
  "event_code": "target_updated|response_rewritten|handshake_success|handshake_failed|sustained_traffic",
  "message": "stable non-sensitive summary",
  "fields": {}
}
```

Messages and fields must not contain credentials, keys, raw traffic, device
addresses, or precise coordinates. Local LuCI views may show the existing
status projection, but event history is deliberately less sensitive.

## Bounds and retention

- Rust WLOC events, including response-rewrite events, are capped at 64 KiB
  and 2 KiB per JSON line. Rewrite events contain byte counters only; they do
  not contain target coordinates or device addresses.
- Provider and redirect events share the same bounded 64 KiB event budget.
  Privacy-safe JSON events have no raw device key, so retention is bounded
  globally by the same recent-record limit.
- Rotation keeps complete newest records and never leaves a partial first
  record for LuCI parsing.
- Debug output is not enabled by the structured event path; normal operation
  emits only state transitions and bounded counters.

## Support bundle

The `support_bundle` rpcd method invokes `/usr/sbin/wloc-support-bundle.sh`.
The helper writes a mode-0600 archive to `/tmp/wloc-support-bundle.tar.gz`,
with a default 64 KiB cap (hard maximum 128 KiB) and a 600-second operational
expiry expectation. Collection is serialized with a lock and the archive is
built in a private temporary directory before an atomic move. It includes only
a manifest, health availability summary, and whitelisted event envelope
fields. It does not copy UCI, status JSON, profile records, raw logs, CA
material, node configuration, or coordinates.

When storage pressure prevents a full event collection, the helper drops the
event files and preserves the manifest/health summary; it never emits an
oversized archive or a partially written tar stream.

## Failure handling

- Missing health/log inputs produce an explicit availability flag, not a fake
  healthy state.
- Malformed/legacy pipe log records are ignored by support-bundle collection.
- A failed bundle generation returns an rpc error and leaves no partial output.
- `support_bundle` is write-only in both packaged ACL sources because it
  creates a file; `health` remains the read-only observation method.
