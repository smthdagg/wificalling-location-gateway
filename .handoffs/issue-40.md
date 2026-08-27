# Agent handoff: Issue 40

## Identity and scope

- Source agent ID: codex-ax6s-oom-fix
- Capabilities used: openwrt,test
- Branch: codex/issue-40-ax6s-oom-lifecycle-codex-ax6s-oom-fix-20260827162459-38c2f99c
- Final local checkpoint before handoff: cd4e4e6
- Credentials included: no

## Objective

Stabilize AX6S package replacement and WLOC auto/manual refresh behavior while preserving the integrated 1.3.0-r1 baseline.

## Completed

- Package lifecycle hooks stop only the managed WLOC and Gateway services before replacement and remove the exact temporary probe directory.
- Lifecycle hooks are executable in generated packages; unrelated Passwall sing-box processes are not stopped.
- Manual WLOC location skips exit/IP probing; automatic mode refreshes bounded evidence and Geo only when required.
- Reused HTTP/2 proxy connections read the current patch target per request.
- LuCI and documentation reflect the manual/automatic location contract.

## Verification

- `./scripts/ci/verify.sh` passed: Python, Rust, JavaScript, packaging, secret scan, advisory scan, and repository gates; total coverage 81.41%.
- Pinned AArch64 OpenWrt cross-build passed for `wloc-service` and `wloc-ctl`.
- AX6S real `opkg --force-reinstall` passed with exit code 0; no OOM counter increase, WLOC/Gateway health green, exactly the expected managed and Passwall sing-box processes, and no temporary probe residue.

## Failed attempts

- The first generated package had non-executable lifecycle hooks and was rejected during installation; the builder now sets hook mode to 0755 and the package test enforces it.
- A pinned Rust container digest was unavailable locally at first; pulling that exact digest resolved the cross-build check without changing the pin.

## Next executable steps

- Review the branch and merge the pull request into `main` after GitHub CI is green.
- Keep the current 1.3.0-r1 package baseline unless a separately approved version bump is requested.

## Capabilities required for the next Agent

- `openwrt` for package and router validation.
- `test` for CI and release-gate verification.

## Security and privacy notes

- No router credentials, tokens, raw production traffic, device identifiers, precise locations, or private keys are included in this capsule or the commits.

## Risks and rollback

- Existing modified OpenWrt conffiles are preserved by opkg as `.opkg` backups.
- Roll back by reverting the PR commit and reinstalling the previous package artifact.
