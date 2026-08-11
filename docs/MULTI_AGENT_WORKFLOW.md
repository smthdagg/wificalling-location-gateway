# Multi-Agent workflow

## Control plane

GitHub Issues are the task queue. Labels express role, phase, and state; Milestones express release gates. The repository avoids a custom scheduler so humans and Agents see the same auditable state.

Task states and leases:

```text
status:triage -> status:ready -> status:active -> status:handoff -> status:active
                                      |                    |
                                      +-> status:review -> status:done
                                      +-> status:blocked
```

Only tasks with explicit owned paths, dependencies, acceptance criteria, non-goals, and rollback behavior may become `status:ready`.

`status:active` mirrors a lease stored under `agent-leases/issue-<n>`. Lease acquisition uses Git's atomic `--force-with-lease` compare-and-swap, so two Agents cannot both publish the same lease generation. `status:handoff` mirrors `agent-handoffs/issue-<n>`. Issue labels and comments are readable projections only; the Git state refs and referenced source commit are authoritative.

## Parallel work lanes

| Lane | Can start now | Gate |
|---|---|---|
| Protocol evidence | fixture schema, capture sanitization, protocol ADR | authorized samples required for parser implementation |
| Security | threat model, CA lifecycle ADR, secret scanning policy | independent review required |
| Exit/Geo | provider interfaces, schema and cache tests | no dependency on WLOC patch code |
| OpenWrt | nftables namespace and rollback design | no live redirect before IPv6 decision |
| Test infrastructure | CI, fuzz harness skeleton, resource measurement | fixtures remain synthetic until authorized |
| Engine | interfaces and limits ADR only | implementation blocked by protocol and license ADRs |

## Starting an Agent

1. Select an Issue carrying `status:ready` or `status:handoff`.
2. Confirm that the Agent satisfies all `cap:*` labels. Capabilities describe tools and expertise, never API keys.
3. Start or take over the task:

   ```sh
   ./scripts/agent-takeover.sh 12 agent-terra fixture-contract 'protocol,test,security' 120
   ```

4. The script leases the Issue and creates a new isolated continuation branch from either `origin/main` or the latest published handoff commit.
5. Work in the generated worktree. Update `.handoffs/issue-12.md` after every meaningful checkpoint.
6. Before stopping, commit all resumable state and release it:

   ```sh
   ./scripts/agent-handoff.sh 12 agent-terra 'protocol,test,privacy'
   ```

7. Another capable Agent can run `agent-takeover.sh`; it starts from the exact published SHA, not from chat memory.
8. When complete, open a PR and move the Issue to `status:review`.

## Handoffs

If a task discovers work outside its owned paths, it opens a new Issue. Cross-role contracts must be committed as schema, interface, fixture, or ADR before dependent implementation begins. Chat messages are coordination hints, not durable requirements.

Every handoff capsule records:

- source Agent ID and non-secret capabilities;
- branch and checkpoint parent;
- completed work and changed files;
- exact verification commands and results;
- failed attempts and unresolved decisions;
- next executable steps and required capabilities;
- environment assumptions, blockers, and security notes.

The capsule must explicitly state that no credentials are included. Uncommitted changes are not transferable state.

## Merge policy

- Required checks: `verify` and `pull-request-contract`.
- At least one approving review.
- Code owner review for CA, proxy, OpenWrt, or workflow paths.
- Resolve all review threads and preserve linear history.
- Delete merged branches; prune worktrees locally after merge.
- PRs must reference `.handoffs/issue-<number>.md` so a reviewer or replacement Agent can reproduce the final state.

## Current GitHub account limitation

The repository is private. GitHub rejected server-side branch protection for the current account tier, so CI, CODEOWNERS, and this contract are active but cannot technically prevent the repository owner from pushing directly to `main`. Upgrade the owner to GitHub Pro before adding autonomous write-capable Agents if hard enforcement is required.
