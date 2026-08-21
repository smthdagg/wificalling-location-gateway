# WLOC fail-open and recovery invariants

Status: Historical Phase 0 contract. The implementation status is governed by
the V2 addendum in `DEVELOPMENT_TEST_PLAN.md`, the executable supervisor and
redirect tests, and the AX6S acceptance evidence. This file remains the
security-invariant source for fail-open behavior; its Phase 0-only wording
must not be read as evidence that the V2 implementation is absent or that
real-device acceptance has passed.

Fail-open means that a failure removes the dedicated interception path for subsequent connections or returns an already authenticated response without modification. It never means weakening TLS verification, inventing a location, accepting untrusted bytes, or promising that an in-flight connection survives.

## Non-negotiable response rule

There is never a default or fallback coordinate. Invalid, missing, conflicting, stale, wrong-exit, unknown, malformed, oversized, or unsupported data cannot create a patch. A verified original response is an Apple response obtained only after successful upstream certificate and hostname verification and preserved without protocol modification.

<!-- SECURITY_INVARIANT id="FAILOPEN-ENGINE" -->

`engine_unhealthy`: remove redirect before stopping or restarting the engine. The external supervisor first blocks new installs, atomically removes the fully named `wificalling_location` redirect/table objects, verifies absence, drains or terminates the process, and cleans bounded temporary state. It never flushes the global ruleset and never changes unrelated provider or firewall state.

<!-- SECURITY_INVARIANT id="FAILOPEN-WATCHDOG" -->

`watchdog_unhealthy`: stop renewing the kernel-expiring lease; active stop also removes redirect. A PID alone is not health. Loss of the external supervisor, missed service probes, crash loops, OOM, startup failure, or uncertain rule ownership makes the disabled/no-live-lease state authoritative. Respawn is bounded and cannot renew a lease until health, scope, CA state, and the reviewed IPv6 mode all pass.

<!-- SECURITY_INVARIANT id="FAILOPEN-LEASE" -->

Every redirect is gated by membership of the assigned-device key in short-TTL nft set elements. The supervisor renews the lease only while engine, scope, and IPv6 health pass. On supervisor death or loss of scheduling, renewal ceases and the kernel automatically expires the lease without help from a userspace process. A rule may remain present but cannot redirect without a matching live lease; startup and reboot begin with no lease.

The lease is defense in depth, not a substitute for deterministic cleanup:
active stop still deletes the redirect and verifies absence before engine
shutdown. V2 implements the bounded lease in the OpenWrt helpers and covers
its expiry/cleanup contract in repository tests; AX6S timing under engine
`kill -9`, supervisor `kill -9`, OOM, scheduler delay, and crash-loop
conditions remains a real-device release gate.

<!-- SECURITY_INVARIANT id="FAILOPEN-GEO" -->

`geo_invalid_or_unavailable`: forward the verified original response without modification. A cache may be used only while unexpired and bound to the same node and current verified exit IP. Invalid ranges/schema/timezone, provider disagreement, WAN-IP observation, exit changes, expiry, or clock uncertainty cannot create a default coordinate.

<!-- SECURITY_INVARIANT id="FAILOPEN-PROTOCOL" -->

`protocol_unknown_or_invalid`: forward the verified original response without modification. Unknown versions/fields without a provably safe round-trip, malformed or truncated input, limit violations, decompression bombs, parser errors, and patch errors are never guessed or partially modified. The body is not logged or cached on failure.

<!-- SECURITY_INVARIANT id="FAILOPEN-TLS" -->

`upstream_tls_invalid`: controlled failure; never disable certificate verification. An invalid chain, unknown CA, expired/not-yet-valid certificate, hostname mismatch, ALPN mismatch, or upstream verification error yields no patched or forwarded attacker response. Only a minimal error category and an exact approved hostname may be audited.

<!-- SECURITY_INVARIANT id="FAILOPEN-IPV6" -->

`ipv6_mode_not_ready`: do not install redirect. The same rule applies when DNS/destination sets, assigned-device identity, CA state, service health, or watchdog ownership is unknown. IPv4-only readiness cannot silently override an unreviewed IPv6 path, and IPv6 must not be disabled globally.

## Ordered lifecycle

Enable order is fail-closed until the final step:

1. validate exact device, exact hostname policy, TCP 443, configuration limits, and the dedicated WLOC table name;
2. verify the chosen IPv4/IPv6 mode and current A/AAAA set ownership;
3. verify router-local CA permissions and exact-SAN policy without exporting private keys;
4. start the engine in pass-through mode and prove TLS/H2/upstream health;
5. arm and prove the external watchdog while the kernel-expiring lease is absent;
6. install the dedicated redirect with a mandatory live-lease match and verify its exact scope;
7. create/renew the short lease only after every prior gate remains healthy.

Disable or fault order always prioritizes network recovery:

1. stop lease renewal and remove the live lease element;
2. prevent new redirect installation;
3. atomically remove the dedicated redirect and verify absence;
4. disarm watchdog ownership only after absence is established;
5. drain or terminate the engine;
6. remove only WLOC temporary sets, leaf cache, and process state.

If redirect absence cannot be verified, the operation reports a hard degraded state and does not claim recovery. Reboot is a final recovery mechanism for the `/tmp` PoC, not a substitute for tested stop/rollback behavior.

## Failure matrix

| Failure | Position/response behavior | Network behavior | Required evidence before live testing |
|---|---|---|---|
| Geo provider unavailable with matching unexpired cache | Use only a policy-approved cached city-level result | Keep healthy scoped service | cache binding and expiry tests |
| Geo invalid, conflicting, stale, or absent | Return authenticated original bytes unchanged | Keep healthy scoped service | schema/conflict/expiry/wrong-exit tests |
| Protocol unknown, invalid, failed, or over limit | Return authenticated original bytes unchanged when safely available | No retry amplification | round-trip, malformed, oversize, bomb, and fuzz tests |
| Apple upstream TLS or ALPN invalid | No position and no unverified response | Controlled failure for that request | negative chain/hostname/ALPN tests |
| Engine health fails while supervisor runs | No new MITM | Stop renewal, remove lease, then actively remove redirect before process action | engine kill/OOM/startup/crash-loop tests |
| Supervisor dies or cannot run | No new MITM after lease expiry | Kernel expires the short lease; a residual rule is inert without a matching lease | supervisor kill/OOM and measured lease-expiry tests |
| DNS set, device binding, CA, or IPv6 mode uncertain | No MITM | Keep redirect absent | state-gate and negative isolation tests |

## Recovery service-level gate

The V2 implementation uses a bounded lease and repository failure-path tests;
the AX6S release evidence must still record the numeric expiry and maximum
blackhole observation under engine `kill -9`, supervisor `kill -9`, OOM,
startup failure, watchdog loss, scheduler delay, crash loop, stop, rollback,
dnsmasq reload, and reboot. No real-device gate may pass with an unmeasured
or aspirational recovery bound.

Audit records may contain only bounded categories, generations, coarse time, exact approved hostname, and byte counts. They never include response/request bodies, precise coordinates, device addresses, CA/leaf keys, node credentials, or provider tokens.

This behavior does not guarantee emergency-call location, does not certify carrier compliance, and does not prove Wi-Fi Calling activation. It applies only to the authorized test device and LAN after all later gates are independently approved.
