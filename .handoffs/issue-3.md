# Agent handoff: Issue 3

## Identity and scope

- Source agent ID: codex-threat-close
- Capabilities used: security,docs
- Branch: codex/issue-3-threat-invariants-codex-threat-close-20260811112704-8048fa7f
- Checkpoint parent: a68bc55693309629510a4f8c873b0cf80587740c
- Updated at (UTC): 2026-08-11T11:38:22Z
- Credentials included: no

## Objective

Freeze the WLOC router PoC threat model and executable fail-open invariants before any live redirect, CA, private-protocol handling, or real-device testing.

## Completed

- Defined exact one-device, two-hostname, TCP 443 scope and Gateway/IPsec isolation.
- Mapped every Critical/High threat to a control and future evidence owner.
- Specified kernel-expiring nft lease recovery independent of the userspace watchdog.
- Defined TLS, ALPN, HTTP/2, CA, Geo, IPv4/IPv6, resource, privacy, emergency, and compliance gates.
- Added code-owned invariant oracles and mutation-style negative tests.
- Obtained independent security review with no remaining P0/P1/P2.

## Files changed

- `docs/security/threat-model.md`
- `docs/security/fail-open.md`
- `tests/security/__init__.py`
- `tests/security/security_invariants.json`
- `tests/security/test_security_invariants.py`
- `.handoffs/issue-3.md`

## Verification

| Command | Result | Evidence |
|---|---|---|
| `python3 -m unittest discover -s tests/security -p 'test_*.py'` | Passed | 12/12 security tests |
| `./scripts/ci/verify.sh` | Passed | 18/18 Python tests, handoff tests, secret scan |
| `git diff --check` | Passed | No whitespace errors |
| Independent security review | Approved | `agent_id=issue3_security_review capabilities=security,test verdict=APPROVE` |

## Failed attempts

- Initial CI discovery did not recurse into `tests/security`; an owned `__init__.py` made the tests part of the full gate.
- Initial watchdog model depended on the process that could itself die; replaced with kernel-expiring lease semantics.
- Initial tests could co-weaken JSON and Markdown; added independent hard-coded oracles and mutation tests.

## Unresolved decisions and blockers

- A later implementation Issue must freeze the nft lease TTL and maximum redirect-removal/blackhole bound, then measure them on AX6S.
- Issue #6 must choose complete dual stack or scoped WLOC-only AAAA suppression before redirect implementation.
- This checkpoint does not authorize certificates, router rules, protocol patching, live traffic, or device testing.

## Next executable steps

1. Review and merge this documentation/test PR.
2. Complete Issue #2 fixture governance and Issue #6 traffic-isolation ADR.
3. Implement only synthetic/offline service slices until every applicable gate is closed.

## Capabilities required for the next Agent

- security
- openwrt
- network
- test

## Environment assumptions

- Python 3 and POSIX shell are available for offline verification.
- No router, device, credential, CA key, provider account, or private fixture is needed.

## Security and privacy notes

- No API keys, tokens, private keys, raw captures, device identifiers, precise locations, or production traffic are included.
- Phase 0 records requirements only; it does not claim that future nft, watchdog, TLS, or CA controls are implemented.
