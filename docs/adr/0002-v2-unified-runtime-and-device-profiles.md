# ADR 0002: v2 unified WLOC runtime and device profiles

Status: Superseded by ADR 0003 (2026-08-22)

> This ADR captured the earlier Gateway/WLOC integration direction. Its useful
> constraints (one supervisor, bounded profiles, fail-open behavior, and
> rollback) remain, but all Gateway package, UCI, lifecycle, and UI coupling is
> removed by [ADR 0003](0003-standalone-wloc-product-boundary.md).

## Context

The current branch still contains historical Gateway adapters, while WLOC has
one global device, node, location mode, and patch target. v2 must keep the AX6S
resource profile while adding independent device records, unified control,
observability, updates, and rollback without depending on the Gateway project.

The optional sing-box provider must not be duplicated per device. The existing
WLOC state machine, probe, Geo, TLS/H2, and protocol code should be reused
behind a unified supervisor rather than rewritten without evidence.

## Decision

v2 will provide one standalone WLOC supervisor and one management plane:

- one procd entry point: `wificalling-location-gateway`;
- one root-only control socket and versioned API;
- one authoritative configuration model containing device profiles;
- one lifecycle state machine coordinating provider, WLOC, redirect, and health;
- one unified status, event, diagnostic, and update surface;
- sing-box remains a managed child process and is shared by profiles that use
  the same node;
- the package resolves an existing `sing-box-tiny`, `sing-box-lite`, or
  PassWall-provided executable before falling back to a normal system
  `sing-box`; it does not copy a second binary or attach to a PassWall-owned
  process;
- no per-device WLOC process and no duplicated sing-box configuration.

Legacy WLOC init scripts and UCI fields may remain as a one-release migration
facade, but they must not independently own runtime state after migration.

## Device profile contract

Each profile owns:

- stable profile id and display label;
- LAN identity and source address set;
- WLOC enabled state;
- one explicit WLOC node binding;
- `auto` or `manual` location mode;
- manual coordinate/preset reference;
- probe, Geo, redirect, proxy, log, and health state.

The runtime must reject an incomplete or ambiguous profile. It must not choose
an arbitrary node when the configured binding cannot be resolved.

## Lifecycle contract

Enable order is:

1. validate profile and scope;
2. prepare Gateway/sing-box in passthrough mode;
3. start or verify WLOC proxy;
4. verify health and the selected IPv6 policy;
5. install only that profile's exact redirect last.

Disable, crash recovery, and rollback must withdraw redirect before draining or
stopping the engine. The status API must be derived from observed runtime state,
not from a successful request to an adapter that performs no operation.

## Resource contract

The v2 release is not accepted until AX6S measurements establish and pass
budgets for:

- unified service idle and active RSS;
- peak RSS while probing through a temporary sing-box instance;
- RSS growth per additional profile;
- CPU during normal monitoring and node switching;
- file descriptors and concurrent H2 work;
- package payload and update staging space;
- log, cache, and temporary-file growth.

The implementation must use bounded queues, bounded caches, bounded logs,
single-flight updates/probes, shared node processes, and atomic cleanup of
temporary artifacts.

## Observability contract

All standalone WLOC events use one structured envelope with timestamp, level,
component, profile id, event, outcome, reason code, retryability, and redaction
version. Raw WLOC bodies, credentials, tokens, and private keys are excluded
by default. Debug capture is explicit, time-limited, size-limited, and
automatically removed.

The UI must expose both the current state and the reason for a degraded state.
Health must cover Gateway, sing-box, node binding, WLOC proxy, CA, Geo freshness,
IPv4/IPv6 policy, redirect presence, and the latest successful operation.

## Migration and rollback

Migration must:

1. back up both v1 UCI files and record hashes;
2. convert the current device/node/location settings into profiles;
3. preserve the CA and compatible runtime data;
4. validate the new configuration before enabling it;
5. keep Wi-Fi Calling in passthrough if WLOC migration fails;
6. remove stale WLOC rules before rollback;
7. restore the previous package/configuration without deleting the CA.

## Consequences

Positive consequences:

- one authoritative standalone WLOC lifecycle and status model;
- independent multi-device control without global mutable WLOC state;
- lower resource use than one process per device;
- consistent diagnostics, updates, and rollback;
- clear separation between the product and its optional sing-box provider.

Costs and risks:

- migration must be tested against existing v1 configurations;
- the unified supervisor becomes a safety-sensitive integration boundary;
- API v2 and LuCI must be versioned together;
- real AX6S resource measurements are required before release.

## Required follow-up

- Update the v1 API document or add a reviewed v2 API document before changing
  the wire contract. The current code already exposes methods not listed in the
  frozen v1 document.
- Add a separate runtime adapter issue for the current `StubRuntime`.
- Add OpenWrt, low-resource, migration, and rollback integration tests before
  removing the legacy service facade.
