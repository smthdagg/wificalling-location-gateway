# Agent handoff: Issue 83

## Identity and scope

- Source agent ID: zcode-matrix-budget-r11-20260829
- Capabilities used: test,ci
- Branch: codex/issue-83-matrix-boot-budget
- Checkpoint parent: `ceaa910` (v1.3.0-r11 merge)
- Updated at (UTC): 2026-08-29
- Credentials included: no

## Objective

Raise the install-matrix boot budget from 45 s to 90 s so the OpenWrt 25.12
apk case (which pre-installs the package before init) completes on a
slow-network run, as observed during the v1.3.0-r11 release verification.

## Completed

- `scripts/openwrt/verify-docker-matrix.sh`: boot wait raised to 90 attempts
  with a comment documenting the apk pre-install stall; no production packages
  or scripts changed.

## Verification

- `sh -n` passes; the r11 matrix was executed against the raised budget and
  completed 8/8 (recorded in the release run).

## Failed attempts

- Three release-matrix runs aborted with 'did not finish booting' on the
  25.12 apk case while the network to downloads.openwrt.org was slow; the
  bare container booted in ~5 s, isolating the stall to the apk
  pre-installation phase (measured ~52 s to ubus).

## Next executable steps

- Merge this harness fix so release runs are not network-fragile.

## Capabilities required for the next Agent

- GitHub CLI (`gh`) with write access to the repository.

## Security and privacy notes

- No credentials are included in this capsule or in the repository; the change
  touches only a test-harness timeout.
