# Agent handoff: Issue 6

## Identity and scope

- Source agent ID: codex-network-adr
- Capabilities used: openwrt,security,network
- Branch: codex/issue-6-traffic-isolation-codex-network-adr-20260811114252-b17aaab4
- Checkpoint parent: a68bc55693309629510a4f8c873b0cf80587740c
- Updated at (UTC): 2026-08-13T05:18:00Z
- Credentials included: no

## Objective

Freeze and test the isolated OpenWrt dual-stack traffic model before any live redirect implementation is accepted.

## Completed

- Selected a unified IPv4/IPv6 lifecycle; scoped AAAA suppression remains a superseding-ADR fallback only.
- Defined exact one-device, two-hostname, TCP 443 matching in a dedicated `wificalling_location` table.
- Prohibited Gateway table changes, global flushes, ordinary HTTPS capture, and UDP 500/4500 interception.
- Modeled short-lived kernel leases, health-gated renewal, startup/reboot without leases, and ordered idempotent teardown.
- Added deterministic DNS A/AAAA rotation, lifecycle, rendered-plan, and semantic-delta tests.

## Files changed

- `docs/openwrt/traffic-isolation.md`
- `openwrt/tests/TRAFFIC_ISOLATION_TEST_PLAN.md`
- `tests/network/README.md`
- `tests/network/__init__.py`
- `tests/network/test_traffic_isolation.py`
- `tests/network/traffic_isolation_model.py`
- `.handoffs/issue-6.md`

## Verification

| Command | Result | Evidence |
|---|---|---|
| `python3 -m unittest discover -s tests/network -p 'test_*.py' -v` | Passed | 21/21 network tests |
| `./scripts/ci/verify.sh` | Passed | 27/27 tests plus secret scan |
| `git diff --check` | Passed | No whitespace errors |

## Failed attempts

- The first RED run failed because the traffic-isolation model did not exist.
- The second RED run exposed missing DNS record reconciliation before the minimum model was implemented.

## Unresolved decisions and blockers

- Independent OpenWrt/network/security review is still required; this checkpoint is intentionally opened as a draft PR.
- The 15-second lease, 5-second renewal, and 20-second recovery gate are design targets until measured on AX6S.
- No real nftables, dnsmasq, procd, root, router, or device mutation is included.

## Next executable steps

1. Perform independent network/security review and close all P0/P1/P2 findings.
2. Rebase onto current main and reconcile this model with the Issue 17 implementation.
3. Measure failure recovery and semantic isolation on AX6S before enabling a live redirect.

## Capabilities required for the next Agent

- openwrt
- network
- security
- test

## Environment assumptions

- Python 3 and POSIX shell are sufficient for this offline model.
- Router access and credentials are not required for review.

## Security and privacy notes

- No secrets, keys, raw captures, device identifiers, precise locations, or production traffic are included.
- Passing the model tests does not prove a live router implementation.
