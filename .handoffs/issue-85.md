# Agent handoff: Issue 85

## Identity and scope

- Source agent ID: zcode-matrix-ipsym-r11-20260829
- Capabilities used: test,ci
- Branch: codex/issue-85-matrix-ip-symlink
- Checkpoint parent: `f693ad1`
- Updated at (UTC): 2026-08-29
- Credentials included: no

## Objective

Make the 25.12 apk matrix case deterministic when offline: the package
metadata carries the file prerequisite `/usr/sbin/ip`; without network apk
cannot install ip-full to provide it, exits nonzero, init never starts, and
the matrix aborts. The rootfs ships the busybox ip applet at `/sbin/ip`, so
the matrix now symlinks `/usr/sbin/ip` before `apk add` — mirroring the opkg
branch's faked ip-full dependency.

## Completed

- `scripts/openwrt/verify-docker-matrix.sh`: the apk pre-install command
  provisions `/usr/sbin/ip -> /sbin/ip` (only when absent) before `apk add`.

## Verification

- Reproduced the offline failure (ubus never reached, apk nonzero), then
  verified with the symlink the container boots to ubus (~74 s offline) and
  the matrix case passes; no production packages or scripts changed.

## Failed attempts

- Raising the boot budget to 90 s alone (issue #83) was insufficient: the apk
  exit code, not the boot time, was the remaining failure mode.

## Next executable steps

- Merge so release runs are not network-fragile.

## Capabilities required for the next Agent

- GitHub CLI (`gh`) with write access to the repository.

## Security and privacy notes

- No credentials are included in this capsule or in the repository; the change
  touches only a test-harness command line.
