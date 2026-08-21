# Agent handoff: Issue 30

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: docs,test,ci
- Branch: codex/issue-30-contracts-codex-v2-lead-20260821044039-bb87d7da
- Checkpoint parent: 601097f
- Updated at (UTC): 2026-08-21T04:50:00Z
- Credentials included: no

## Objective

Define the v2.0 unified Gateway/WLOC runtime, device-profile, UI,
observability, update, resource, migration, and release contracts before
implementation.

## Completed

- Added the proposed v2 unified Gateway/WLOC runtime ADR.
- Added the proposed v2 device/profile/control API contract.
- Added V2-00 through V2-09 task ownership and verification gates.
- Added publishable GitHub Issue drafts.

## Evidence

- Planning document sanity check passed.
- Cached source verification remains the repository's next handoff gate.
- No Rust, OpenWrt, LuCI, package, or test implementation files were changed.

## Verification

- Planning document sanity check passed.
- Full repository verification is the next handoff gate.

## Failed attempts

- The first handoff capsule omitted the required validation headings; it was
  corrected before publication. No product code validation failed.

## Next executable steps

1. Run `./scripts/ci/verify.sh` and publish this handoff capsule.
2. Create and lease the first implementation child Issue for the profile/UCI/API
   model.
3. Implement the child Issue on a separate owned-path branch with tests first.

## Capabilities required for the next Agent

- docs
- test
- ci
- rust

## Security and privacy notes

- No credentials, private keys, raw production traffic, device identifiers, or
  precise location data are included.
- The v2 contracts preserve LAN scope, fail-open behavior, exact-host/TCP443
  interception limits, and the prohibition on UDP 500/4500 interception.

## Next step

Create and lease the first implementation child Issue for the profile/UCI/API
model, then implement it in a separate owned-path branch after the v2 contracts
are accepted.
