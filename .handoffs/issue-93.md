# Agent handoff: Issue 93

## Identity and scope

- Source agent ID: zcode-upgrade-hygiene-docs-20260830
- Capabilities used: docs,ci
- Branch: codex/issue-93-upgrade-hygiene-docs
- Checkpoint parent: `8e28249` (r13 docs)
- Updated at (UTC): 2026-08-30
- Credentials included: no

## Objective

Write the post-upgrade memory/temp hygiene and the private signed feed
update into the project's standard documents, after the live incident where
an upgrade IPK left in /tmp pushed MemAvailable below the cold-start memory
gate, and after the feed index generator was found to be documented at a
path that does not exist in this repository.

## Completed

- `AGENTS.md` (AX6S upgrade and debugging cleanup gate): the signed-feed
  upgrade is now the standard path (creates no /tmp artifact); local IPK
  installs must delete the file in the same command chain; ad-hoc debug
  backup directories must not survive the session; the post-test memory
  requirement now references the computed cold-start gate of
  `require_start_memory` (inflated Lite runtime + 8 MiB) instead of the old
  flat 32 MiB; `drop_caches` is allowed only as a final measurement
  normalization after zero leaked files/processes are confirmed.
- `docs/releases/RELEASE_PROCESS.md`: mandatory post-upgrade hygiene step 5a
  in the release checklist (remove uploaded installers and session backup
  dirs, normalize the measurement, verify MemAvailable above the cold-start
  requirement and service health).
- `README.md` (bilingual): AX6S upgrade guidance now says /tmp is RAM,
  prefers the feed upgrade, and requires delete-after-install for local IPKs.
- `docs/releases/RELEASE_PROCESS.md` step 6 and `AGENTS.md` required
  workflow now include the private signed feed update as a mandatory
  standard step: the index generator lives in the feed repository
  (`scripts/gen-feed-index.sh` on the feed main branch), index regeneration
  before signing is mandatory, the feed `README.md` package table must be
  aligned with the current release, and feed pushes require the account's
  noreply git identity.

## Verification

- `./scripts/ci/verify-handoffs.sh` passes; docs-only change, no production
  code touched. The live incident that motivated this standard is documented
  in `docs/testing/V1.3.0_R13_MEMORY_GATE.tdd.md`.

## Failed attempts

- None; the three documents applied cleanly.

## Next executable steps

- Merge this docs PR. The workflow applies to every future live upgrade and
  to release steps 5/5a.

## Capabilities required for the next Agent

- GitHub CLI (`gh`) with write access to the repository.

## Security and privacy notes

- No credentials are included in this capsule or in the repository.
