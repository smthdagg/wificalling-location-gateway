# Development readiness report

Date: 2026-08-11

> Historical Phase 0 snapshot. This report predates the V2 Rust/OpenWrt
> implementation and is retained for audit history only. Current status is in
> [`V2.0_TASK_BREAKDOWN.md`](../releases/V2.0_TASK_BREAKDOWN.md),
> [`OPENWRT_PACKAGE_DOCKER_MATRIX.md`](OPENWRT_PACKAGE_DOCKER_MATRIX.md), and
> [`AX6S_RESOURCE_EVIDENCE.template.md`](AX6S_RESOURCE_EVIDENCE.template.md).
> Do not use the old `implementation=BLOCKED` result as the current release
> decision; real AX6S migration/resource/rollback evidence is still pending.

## Result

| Layer | Result | Meaning |
|---|---|---|
| Multi-Agent coordination | READY | The task, lease, handoff, CI, secret-scan, and local verification baseline can be used. |
| Offline Go safety scaffold | READY | Go 1.23 module, generic metadata gate, tests, coverage, and pinned offline Docker verification are available. |
| WLOC protocol implementation | BLOCKED | No authorized fixture or private-protocol evidence has been approved; parser, patch, CA, MITM, and live traffic remain prohibited. |

This distinction is intentional. A green coordination CI run does not authorize WLOC parser, TLS interception, response patching, or real-device MITM work.

## Reproduce

```sh
python3 scripts/dev_readiness.py --profile coordination
python3 scripts/dev_readiness.py --profile implementation
./scripts/ci/verify.sh
```

The implementation profile exits with status `2` while any required gate is missing. Add `--json` for machine-readable output.

## Remaining blockers

- `go` is not installed in the current Agent environment.
- `shellcheck` is not installed locally. GitHub Actions still provides the remote ShellCheck gate.
- No actual `authorized-sanitized-capture` fixture has passed provenance, sanitization, and review.
- Authorized hostname/TLS/ALPN/H2 behavior evidence and a private-protocol contract do not exist.
- The IPv6 approach and fail-open/watchdog recovery SLO remain undecided.

Later system-test dependencies are also absent locally: an ARM64 QEMU runtime and a discoverable OpenWrt/ImmortalWrt SDK. They do not block documentation and fixture-governance work, but they block packaging and emulated system validation.

Python `coverage` is not installed, so the Python coordination helpers still have no numeric coverage claim. The Go gate enforces an 80% minimum and currently reports 87.0% statement coverage.

## Work allowed now

1. Create only synthetic manifest-validator cases under the accepted governance contract.
2. Obtain and independently approve authorized protocol evidence without committing private captures.
3. Freeze the IPv6 approach and fail-open/watchdog recovery SLO.
4. Keep Go work limited to generic offline safety contracts until those gates pass.

## Work not allowed yet

- Copying or adapting AGPL WLOC implementation code into an MIT codebase.
- Implementing WLOC response parsing or patching from undocumented/private captures.
- Installing a CA on a real iPhone or redirecting real WLOC traffic.
- Treating missing Geo/protocol data as a fixed default location.
- Starting OpenWrt packaging or device deployment before rollback, IPv6, and CA lifecycle gates exist.

## Verification evidence

- TDD RED: the new readiness tests initially failed because `scripts/dev_readiness.py` did not exist.
- TDD GREEN: five readiness tests pass, including deterministic missing-tool, missing-artifact, and unaccepted-document cases.
- TDD RED/GREEN follow-up: readiness now blocks on Phase 0 documents that merely exist but are not explicitly Accepted.
- AI regression review removed an environment-dependent mock that could have produced an accidental pass.
- Full repository verification passes 11 Python tests, shell/Go verifier tests, secret scanning, syntax checks, and `git diff --check`.
- Go 1.23.12 tests pass with an enforced 87.0% statement coverage result through the digest-pinned, offline Docker fallback.
- The latest remote `main` run for commit `a68bc55` completed successfully before these local changes.

No remote branch, Issue, commit, PR, package, credential, or system dependency was changed during this preparation pass.
