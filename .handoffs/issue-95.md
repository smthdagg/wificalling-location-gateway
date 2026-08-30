# Agent handoff: Issue 95

## Identity and scope

- Source agent ID: zcode-feed-restructure-docs-20260830
- Capabilities used: docs,ci
- Branch: codex/issue-95-feed-restructure-docs
- Checkpoint parent: `d6336e0`
- Updated at (UTC): 2026-08-30
- Credentials included: no

## Objective

Align all documentation with the feed restructure: the feed repository is
renamed to `Smthdagg-Repo-feeds` and reorganized into per-project
subdirectories (each project gets its own index and signature; the directory
name must equal the project repository name), with a master update log and a
verifier enforcing that every index change is recorded.

## Completed

- Feed repository renamed and restructured (gh-pages pushed, 1545651):
  `wificalling-location-gateway/` holds the four r13 IPKs with a freshly
  generated and signed 4-entry index; eight project directories reserved
  with stub READMEs; root `README.md` documents the directory-per-project
  rule, the exact per-project update procedure, and router feed lines;
  root `UPDATES.md` is the master update log (historical entries backfilled);
  `scripts/feed-verify.sh` verifies index integrity, signatures, checksums,
  and that `UPDATES.md` covers every changed project.
- Feed `main` branch README install URLs updated (310f436).
- AX6S migrated to the new feed URL: `opkg update` prints `Signature check
  passed` and the r13 packages are visible; installed version unchanged
  (1.3.0-r13).
- This repository's docs updated to the new flow: `RELEASE_PROCESS.md` step
  6 (feed-repo workflow: per-project subdir, index-before-sign, UPDATES.md,
  feed-verify, README alignment, noreply identity), `AGENTS.md` 7a (feed
  update in the required agent workflow), `README.md` install URLs
  (bilingual).

## Verification

- `feed-verify.sh` on the restructured feed: OK (it also caught and
  fixed a checksum-cwd bug in its own first version, and the initial
  restructure missed the two Lite IPKs — both corrected before push).
- AX6S `opkg update` signature check passed against the new URL.
- Full `./scripts/ci/verify.sh` gate green on this docs branch.

## Failed attempts

- The first gh-pages restructure moved only the two Standard IPKs (the shell
  glob did not match the `-lite_` filenames) and shipped an index missing the
  Lite packages; caught by the new verifier and corrected.
- The verifier's first version checked checksums from the feed root instead
  of the project subdirectory; fixed before push.
- `git commit --reset-author` without `--amend` failed twice; fixed by
  setting the repository-local noreply identity.

## Next executable steps

- Merge this docs PR; no release actions required.

## Capabilities required for the next Agent

- GitHub CLI (`gh`) with write access to `smthdagg/wificalling-location-gateway`
  and `smthdagg/Smthdagg-Repo-feeds`.

## Security and privacy notes

- No credentials are included in this capsule or in the repository.
- The signing key policy is unchanged: same long-lived key, private key never
  leaves the release machine.
