# ADR: isolated OpenWrt traffic and IPv6 lifecycle (historical boundary)

Status: historical Phase 0 boundary. The current standalone WLOC lifecycle is
defined by ADR 0003; this ADR and its
tests are offline design evidence only. They do not authorize or contain live
`nft`, `dnsmasq`, `procd`, router, CA, MITM, or device operations.

## Decision

Use one complete dual-stack lifecycle. IPv4 and IPv6 each have an assigned
device set, an exact-WLOC destination set, a kernel-expiring live-lease set,
and a redirect predicate. Both address families must be healthy before either
can intercept. There is no IPv4-only degraded interception mode.

The alternative—suppressing AAAA only for the assigned device and the two WLOC
names—is rejected for this design. The known AX6S constraints (memory, storage,
and existing IPv4-only Gateway policy) do not establish that two small IPv6
sets and a symmetric rule are infeasible. Full dual stack also avoids a special
DNS answer path and preserves ordinary IPv6 semantics. A later AX6S capability
test must still prove timeout elements and IPv6 redirect behavior. If that test
fails, interception stays disabled; switching to scoped AAAA suppression
requires a superseding reviewed ADR and new negative-isolation tests.

## Scope and ownership

The future implementation may own only the fully named `inet`
`wificalling_location` table and objects within it. It must never reuse,
modify, flush, rename, or depend on `wificalling_gateway`, the global ruleset,
another table, or the running sing-box configuration.

A connection is eligible only when every predicate is true:

1. its source is the currently assigned single test device in the matching
   IPv4 or IPv6 source set;
2. its destination is a current, unexpired A or AAAA address learned from
   exactly `gs-loc.apple.com` or `gs-loc-cn.apple.com`;
3. it is TCP destination port 443;
4. the matching assigned-device key has a live kernel timeout element; and
5. TLS ingress independently confirms one exact approved hostname before any
   leaf signing or proxying.

An IP set is routing containment, not hostname authentication. Shared or
poisoned destination addresses therefore cannot bypass the independent TLS
hostname check. Other devices, ordinary HTTPS, router management, sing-box
management/health traffic, and every UDP flow—including IPsec ports 500 and
4500—bypass the component.

## Declarative object plan

Issue #6 deliberately freezes semantics rather than executable syntax. The
machine-readable renderer in `tests/network/traffic_isolation_model.py` emits
`offline-review-manifest/v1` dictionaries with `executable: false`. Future
OpenWrt code must be derived in a separate implementation Issue and reviewed
against these manifests.

The table has symmetric logical objects:

| Purpose | IPv4 | IPv6 | Required behavior |
|---|---|---|---|
| assigned source | one address | one address | drift disables both families |
| WLOC destination | current A set | current AAAA set | exact-name observations, TTL expiry |
| live lease | assigned-device key | assigned-device key | kernel timeout element |
| redirect | scoped predicate | scoped predicate | requires the live lease |

No object has a wildcard source, wildcard hostname, default route prefix, UDP
match, or broad HTTPS match. Rule handles are implementation details; teardown
resolves only objects carrying the project-owned names and refuses ambiguous
ownership.

## DNS rotation and reload

Only answers originating from the two complete names may enter a generation.
Only syntactically valid A and AAAA records are accepted. Each element expires
at its authoritative record TTL; expired addresses cannot be copied into a new
generation. An update builds both family sets off-path and atomically replaces
the owned generation, so address rotation cannot mix old A records with new
AAAA records.

A dnsmasq reload rebuilds the owned generation from fresh exact-name answers.
Until both family paths are ready, the live lease is absent. Empty A or AAAA is
not silently treated as complete dual-stack readiness; an explicitly proven
IPv6-only or IPv4-only upstream condition must be represented by the later
implementation state machine and independently tested before it can enable.
No global IPv6 setting or answer is changed.

Device IPv4 or IPv6 address/lease drift is a scope fault: stop renewal, remove
both live leases and the redirect, then repopulate both source sets from the
validated binding. An address learned for a second LAN device never enters the
sets.

## Kernel lease and recovery bound

The design freezes a 15-second kernel element TTL and a 5-second healthy
renewal cadence. Startup and reboot create no live lease. The external
supervisor renews only while the engine, exact scope, DNS A and AAAA
generations, both address-family paths, and watchdog ownership are healthy.
A PID or procd respawn state is not health.

If the engine fails while the supervisor runs, the supervisor stops renewal,
removes both lease elements, then removes and verifies absence of the owned
redirect before any engine restart. If the supervisor is killed, OOM-terminated,
wedged, or unscheduled, renewal stops; the kernel expires the element and any
residual rule becomes inert without userspace help.

The offline model proves ordering, not AX6S timing. Before real-device testing,
kernel tests must measure engine kill, supervisor kill, OOM, scheduler delay,
and crash-loop cases. Acceptance is expiry no later than 20 seconds after the
last successful renewal, with the raw maximum recorded. Any miss leaves the
feature disabled and requires a reviewed bound change; it cannot be waived by
calling reboot the rollback mechanism.

## Ordered lifecycle

Enable is fail-closed until its final two operations:

1. validate one device, two exact names, TCP 443, object ownership, and bounds;
2. build and validate current A and AAAA generations;
3. prove the unified IPv4/IPv6 path and device binding;
4. start the engine in pass-through mode and prove engine health;
5. arm the external watchdog while the lease is absent;
6. install the owned redirect last, with mandatory live-lease membership;
7. create or renew the lease only while every earlier gate remains healthy.

Disable and active fault handling prioritize recovery and are idempotent:

1. stop renewal and delete both live-lease elements;
2. block new installs;
3. remove only the owned redirect and verify semantic absence;
4. disarm watchdog ownership;
5. drain or terminate the engine;
6. remove owned destination/source sets and temporary state as requested.

Repeated disable with every owned object absent succeeds without changing any
unrelated state. Failure to verify absence reports hard degraded state and does
not claim recovery.

## procd boundary

The future procd service starts pass-through only and uses bounded respawn. An
external health owner, not the engine and not the PID file, controls renewal.
Respawn cannot recreate a lease, inherit a lease, or install redirect. The
service's start/reload/restart/reboot paths all begin with lease absence and
must re-run every gate. Its stop path follows the disable order above.

## Semantic before/after proof

Every privileged integration test must capture normalized semantic state
before enable, after enable, after fault, after disable, and after reboot.
Normalization includes table/chain/set names, families, hooks, priorities,
predicates, ownership, timeout semantics, and DNS generations while excluding
volatile counters and handles.

The proof passes only when:

- enable adds or changes `wificalling_location` objects and nothing else;
- the normalized `wificalling_gateway` and unrelated-state hashes are equal
  before and after every operation;
- packet tests show redirects only for both approved IPv4/IPv6 cases;
- other devices, ordinary HTTPS, router/sing-box management, and UDP 500/4500
  have zero WLOC redirect hits; and
- stop, kill/OOM recovery, lease expiry, dnsmasq reload, and reboot return to
  the original unrelated-state snapshot.

Text diffs alone are insufficient because handles and counters can obscure a
semantic change. The offline `semantic_delta` test models this allowlist; a
later privileged harness must inspect actual kernel state without global flush.

## Test gates and non-goals

The offline suite verifies renderer safety, exact flow classification, DNS
TTL/rotation, lifecycle order, kill/OOM behavior, watchdog lease expiry, reboot,
idempotent disable, and before/after isolation. The later OpenWrt harness must
repeat these cases in a namespace/QEMU environment before AX6S measurement.

This ADR does not implement rules, edit configuration, require root, generate
certificates, use production traffic, connect a device, or authorize live
router mutation. The WLOC service has no Gateway dependency. Any provider
executable (system sing-box, sing-box tiny, or a PassWall-provided sing-box)
is an explicitly configured capability and is never managed as another product.
