# Multi-agent development contract

## Mission

Build `wificalling-location-gateway` as an independent, fail-open OpenWrt
product containing both the Wi-Fi Calling Gateway and WLOC components. The
repository must not depend on, vendor, or modify the separate upstream Wi-Fi
Calling Gateway 1.7 repository. Its in-repository Gateway and WLOC modules
share one lifecycle, LuCI management surface, logs, monitoring, update path,
and rollback model.

## Source of truth

1. A GitHub Issue is the only unit of assignable work and the durable coordination record.
2. `DEVELOPMENT_TEST_PLAN.md` defines architecture, safety gates, and phase exit criteria.
3. The Issue defines the owned paths, dependencies, acceptance tests, and non-goals.
4. A pull request is the only integration path into `main`.

## Agent roles and default ownership

| Role label | Default paths | Responsibility |
|---|---|---|
| `role:protocol` | `internal/wloc/`, `fixtures/` | Authorized fixtures, protocol notes, parser/patch behavior |
| `role:engine` | `cmd/`, `internal/ca/`, `internal/proxy/` | Process, TLS, HTTP/2, limits, fail-open behavior |
| `role:network` | `internal/exitprobe/`, `internal/georesolver/`, `openwrt/` | Exit probing, Geo resolution, nftables/dnsmasq/procd |
| `role:security` | `SECURITY.md`, `docs/security/`, `.github/` | Threat model, CA lifecycle, permissions, policy checks |
| `role:test` | `tests/`, `scripts/ci/` | Test harness, fuzzing, packaging and resource gates |
| `role:integration` | `docs/`, packaging metadata, product contract | Gateway/WLOC integration and release documentation |

Issue-specific ownership overrides this table. Ownership is a time-limited lease, not permanent assignment. An Agent must not edit another active lease's paths unless it is performing a recorded takeover from an expired or released lease.

## Identity, credentials, and capabilities

- Each Agent uses its own API key and authentication environment. Never copy credentials between Agents.
- API keys, tokens, `.env` files, provider account names, and credential fingerprints are not handoff data and must not enter GitHub.
- Agents identify themselves with a non-secret `agent_id` and declare only capability tags such as `go`, `tls-h2`, `protobuf`, `openwrt`, `security`, `ios-device`, `ci`, or `docs`.
- An Agent may take over a task only when it satisfies every `cap:*` label or records a limited research/review scope that does not execute the restricted work.
- Lease authority is the atomically updated `agent-leases/issue-<n>` Git ref; handoff authority is `agent-handoffs/issue-<n>`. Issue labels and comments are display-only projections.
- The immutable continuity anchor is the pushed source commit recorded by the authoritative handoff Git ref.
- State refs are a cooperative lock for Agents that already have repository write access, not an authorization boundary. Hard protection requires GitHub Pro branch/ruleset protection or a single-writer coordination service.

## Required workflow

1. Lease one `status:ready` or `status:handoff` Issue with a non-secret Agent ID and capability list.
2. Create `codex/issue-<number>-<slug>-<agent>` in an independent worktree, based on the latest handoff commit when one exists.
3. Read this file, the Issue, and relevant sections of `DEVELOPMENT_TEST_PLAN.md`.
4. Write tests or executable verification before implementation when product code is in scope.
5. Keep commits focused and use Conventional Commits.
6. Before pausing, lease expiry, or PR creation, update `.handoffs/issue-<number>.md`, commit it, push the branch, and publish the exact commit.
7. Open a PR containing `Closes #<number>`, evidence, risks, rollback notes, and the handoff capsule path.
8. A different role reviews the PR. The author never self-approves a safety-sensitive change.

Use `scripts/agent-takeover.sh <issue> <agent> <slug> <capabilities> [ttl-minutes]` to start or resume work. Use `scripts/agent-handoff.sh <issue> <agent> <capabilities>` to release a resumable checkpoint.

## Hard gates

- Do not implement WLOC response patching before the Phase 0 authorized-fixture and license ADR Issues are closed.
- Never commit CA private keys, node credentials, captured device identifiers, raw production traffic, tokens, or precise user location. Local pre-push scanning and CI reduce accidental leaks but cannot stop an authorized writer who bypasses the workflow.
- All parser and network inputs require size, time, concurrency, and schema limits.
- Unknown protocol, invalid Geo data, or engine failure must not produce a default fake coordinate.
- WLOC interception must remain limited to the assigned test device, two exact Apple hostnames, and TCP 443.
- Never intercept UDP 500/4500 for WLOC. The in-repository Wi-Fi Calling
  Gateway retains ownership of its own data-plane rules; the unified product
  supervisor must coordinate both namespaces without cross-component damage.
- Changes under `internal/ca/`, `internal/proxy/`, `openwrt/`, or `.github/workflows/` require security review.

## Verification

Run before every PR:

```sh
./scripts/ci/verify.sh
```

Product code must eventually meet the 80% coverage policy, but an empty scaffold does not fabricate coverage. Each Issue must state the tests appropriate to its phase.
