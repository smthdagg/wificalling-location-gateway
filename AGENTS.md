# Multi-agent development contract

## Mission and release boundary

Maintain `wificalling-location-gateway` as the stable, integrated Wi-Fi Calling + WLOC OpenWrt product. Preserve its fail-open WLOC behavior and the proven `1.3.0-r1` baseline; incremental releases extend that baseline instead of redesigning it. Historical 1.7-era development records are closed as design provenance only and must not be referenced as a baseline or external dependency for future work.

- The `1.3.0-r1` stable integrated release is the permitted build/package baseline; earlier 1.2.x packages and every retired 1.7.x package are not valid build inputs.
- Retired standalone Wi-Fi Calling Gateway packages, including every 1.7.x package, must be rejected as build inputs and must not be copied back into this repository.
- The multi-device/2.0 Beta line is maintained only in the separate Beta repository. It is outside this repository's architecture, source, issues, branches, releases, documentation, packaging, and test scope; do not import or recreate it here.
- The `2.0` value in an IPK `debian-binary` member is an archive-format marker, not a project version, and remains required for valid IPK output.
- Package format is per-platform and is a build/packaging rule, not a user concern: OpenWrt 24.10 `.ipk` must be a whole-file gzip-wrapped tar (a bare tar is rejected by `opkg` as malformed); OpenWrt 25.12 dropped the `.ipk` format, so releases there use the native `.apk` (an `.ipk` on 25.x fails with `v2 package format error`). Pick the asset that matches the target's package manager and never rename an IPK into an APK.

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
| `role:integration` | `docs/`, packaging metadata, Gateway contract | Cross-module contracts and release integration |

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
7a. For every release PR, update the private signed feed (mandatory, before
   tagging): work inside the project's own subdirectory of the feed repo
   `gh-pages` branch (`wificalling-location-gateway/` — directory name must
   equal the project repository name), regenerate that project's index with
   the feed repo's `scripts/gen-feed-index.sh`, sign with
   `scripts/openwrt/sign-feed.sh`, append a row to `UPDATES.md`, run the
   feed repo's `scripts/feed-verify.sh` (must pass), push `gh-pages`, and
   align the feed `README.md` package table with the current release. Full
   sequence: `docs/releases/RELEASE_PROCESS.md` step 6. The feed repo
   requires the account's noreply git identity.
8. A different role reviews the PR. The author never self-approves a safety-sensitive change.

Use `scripts/agent-takeover.sh <issue> <agent> <slug> <capabilities> [ttl-minutes]` to start or resume work. Use `scripts/agent-handoff.sh <issue> <agent> <capabilities>` to release a resumable checkpoint.

## AX6S upgrade and debugging cleanup gate

Every package upgrade or debugging session performed on a real AX6S must end
with the following cleanup and evidence check before the device is handed back
or a release is accepted:

1. Record `free`, `df -h /tmp /overlay`, the relevant process list, and the
   package version before testing. Treat `/tmp` as RAM: uploaded IPKs, extracted
   runtimes, logs, and test outputs consume memory, not just disk space.
2. Keep large upload artifacts in `/root` when possible. If `/tmp` is required,
   maintain an explicit list of files created by the session and remove those
   files after installation/testing, including failed or superseded IPKs. Use
   short, bounded commands or a cleanup trap so SSH command truncation cannot
   silently skip cleanup. The standard upgrade path is the signed feed
   (`opkg update && opkg upgrade <package>`), which creates no `/tmp` artifact
   at all; when a local IPK must be uploaded, the install command chain must
   delete it in the same session (`opkg install /tmp/x.ipk && rm -f /tmp/x.ipk`),
   and any ad-hoc debug backup directory (e.g. `wloc-*-backup-*`) must be
   removed before the session ends.
3. Never run a blanket `rm -rf /tmp` on a live gateway. Preserve the active
   `/tmp/sing-box-lite`, its checksum marker, `/var/run` sockets/configuration,
   PassWall runtime state, and user files; remove only session-owned temporary
   artifacts after verifying their exact paths.
4. After cleanup, verify the package/service version, service health, control
   socket, and steady-state process count. There must be no transient probe,
   duplicate sing-box, shell, curl, or package-install process left behind;
   any second long-lived sing-box owned by an explicitly enabled service such
   as PassWall must be identified and included in the memory budget.
5. Repeat `free` and `df -h /tmp /overlay`. The post-test available memory must
   return to the pre-test baseline within 10 MiB and remain above the computed
   cold-start requirement of `require_start_memory` (inflated Lite runtime +
   8 MiB; roughly 38 MiB on the AX6S); otherwise block the release and
   investigate. Do not use `drop_caches` to hide leaked files or processes;
   after zero leaked files/processes are confirmed, one final
   `sync; echo 3 > /proc/sys/vm/drop_caches` is allowed to normalize the
   measurement before re-reading `MemAvailable`.
6. If OOM or a service crash occurs, save the relevant `logread`/kernel OOM
   evidence before cleanup, identify the triggering operation, then clean and
   re-measure. A test is not complete while stale packages or temporary
   runtimes remain in `/tmp`.

## Hard gates

- Do not implement WLOC response patching before the Phase 0 authorized-fixture and license ADR Issues are closed.
- Never commit CA private keys, node credentials, captured device identifiers, raw production traffic, tokens, or precise user location. Local pre-push scanning and CI reduce accidental leaks but cannot stop an authorized writer who bypasses the workflow.
- All parser and network inputs require size, time, concurrency, and schema limits.
- Unknown protocol, invalid Geo data, or engine failure must not produce a default fake coordinate.
- WLOC interception must remain limited to the assigned test device, six exact WLOC hostnames, and TCP 443.
- Never intercept UDP 500/4500 or modify the integrated WCG nftables table.
- Changes under `internal/ca/`, `internal/proxy/`, `openwrt/`, or `.github/workflows/` require security review.

## Verification

Run before every PR:

```sh
./scripts/ci/verify.sh
```

Product code must eventually meet the 80% coverage policy, but an empty scaffold does not fabricate coverage. Each Issue must state the tests appropriate to its phase.
