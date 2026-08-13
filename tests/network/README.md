# Issue #6 TDD evidence

Journeys were derived from Issue #6, `DEVELOPMENT_TEST_PLAN.md`, and the
reviewed Issue #3 threat/fail-open candidates.

| Guarantee | Test | Type |
|---|---|---|
| only the dedicated table is represented; Gateway/global/IPsec are absent | `RenderedPlanTests` | contract |
| IPv4 and IPv6 use the same exact source/destination/TCP/lease scope | `RenderedPlanTests`, `FlowIsolationTests` | unit |
| exact A/AAAA generations rotate atomically and TTL-expired entries disappear | `DnsRotationTests` | unit |
| startup/reboot have no lease; redirect is installed last | `LifecycleTests` | state model |
| disable is idempotent and network recovery precedes process stop | `LifecycleTests` | state model |
| engine kill/OOM removes redirect; supervisor loss becomes inert on expiry | `LifecycleTests` | failure model |
| before/after proof permits changes only to owned objects | `SemanticProofTests` | contract |

RED evidence:

- initial run failed importing the absent `traffic_isolation_model`;
- DNS rotation extension failed importing absent `Record` and
  `reconcile_dns_generation`.

GREEN command: `python3 -m unittest tests.network.test_traffic_isolation`.
The suite is offline and unprivileged. It does not claim kernel, QEMU, AX6S, or
real-device coverage; those gaps are explicit staged gates in
`openwrt/tests/TRAFFIC_ISOLATION_TEST_PLAN.md`.
