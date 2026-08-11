# Agent handoff: Issue 17

## Identity and scope

- Source agent ID: zcode-wloc-service
- Capabilities used: rust,security
- Branch: codex/issue-17-wloc-service
- Checkpoint parent: 06ea4d80ea29c4d5eaec2bad9af80b4cb05e7ff5
- Updated at (UTC): 2026-08-11T12:45:59Z
- Credentials included: no

## Objective

Continue the WLOC service control-plane implementation by hardening the
local Unix-domain control-frame codec so a half-consumed or hostile peer can
never resume mid-frame after an error. This is the bounded 16 KiB / 2 s
control-frame transport that Issue #6 traffic isolation depends on, evolved
from the earlier free-function codec into a connection-stateful `FramedIo`.

## Completed

- Rewrote `tests/runtime_uds.rs` (RED) around a `FramedIo` struct that owns a
  poisoned flag, defining the stronger contract: `ConnectionPoisoned` after
  any error, sanitized I/O errors (ErrorKind only), and a single total
  per-frame deadline that a slow trickle cannot reset.
- Implemented `FramedIo` in `src/runtime/uds.rs` (GREEN): replaced the
  free-function `read_frame`/`write_frame` API with a struct whose every
  failing operation poisons the connection; subsequent calls return
  `ConnectionPoisoned` without performing further I/O.
- `FrameError` gains `ConnectionPoisoned`; `Display` forwards only the
  `io::ErrorKind`, never the underlying peer-supplied message.
- Empty and oversized payloads are rejected before any I/O and also poison.
- `MAX_CONTROL_FRAME_BYTES` and `CONTROL_FRAME_TIMEOUT` remain the
  transport-layer bounds; `service::api::MAX_CONTROL_FRAME_BYTES` is the
  public contract surface used by callers.

## Files changed

- `src/runtime/uds.rs` - `FramedIo` struct, `ConnectionPoisoned` variant,
  poisoned-connection semantics, single total deadline.
- `tests/runtime_uds.rs` - rewritten frame codec contract tests.

## Verification

| Command | Result | Evidence |
|---|---|---|
| `cargo test --test runtime_uds` | Passed | 9 tests, including anti-trickle deadline, partial-write deadline, flush/read error sanitization, and poisoning after every error class |
| `cargo test --workspace --all-targets` | Passed | 60 tests, 0 failed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | no warnings |
| `cargo fmt --check` | Passed | formatted |
| `./scripts/ci/verify.sh` | Passed | 92.18% line coverage (runtime/uds.rs 85.00%), RustSec advisories/bans/licenses/sources ok, release binary 968,160 bytes, repository gates passed |

## Failed attempts

- None in this checkpoint. The first `verify.sh` run failed only because the
  inherited (unformatted) test file tripped `cargo fmt --check`; `cargo fmt`
  resolved it and the re-run passed.

## Unresolved decisions and blockers

- Issue #17 still has the previously identified 3 P1 + 2 P2 security findings
  (compensation-delete blackhole, WAN-unknown exit acceptance, IPv6
  special-use filtering; state-machine ordering, Geo TTL) to reproduce and
  fix in a later TDD cycle.
- The branch has not yet been opened as a PR; Phase 0 fixture/threat-model/
  license gates and the OpenWrt runtime/system test evidence remain
  prerequisites for merge.
- `main` branch protection still requires GitHub Pro; squash-merge +
  CODEOWNERS + CI + Agent rules remain the compensating controls.

## Next executable steps

1. Reproduce the 3 P1 findings with fault-injection tests (RED), then fix the
   exitprobe/georesolver implementations (GREEN), keeping fail-open safety.
2. Run `./scripts/ci/verify.sh` and push this branch so the remote CI mirrors
   the local gates.
3. Once Phase 0 gates close, open the Issue #17 PR with `Closes #17`, the
   handoff capsule path, evidence, risks, and rollback notes; a different
   role reviews it.

## Capabilities required for the next Agent

- rust
- security

## Environment assumptions

- Rust 1.90.0 (MSRV), cargo, cargo-audit, cargo-deny, and llvm-cov are
  available on the host.
- No network access, device, CA, or production fixture is required for this
  offline control-frame codec work.

## Security and privacy notes

- No API keys, tokens, private keys, `.env` values, raw captures, device
  identifiers, or precise user locations are included.
- I/O error messages from the underlying transport are dropped at the codec
  boundary; only `io::ErrorKind` crosses into `FrameError`.
- No WLOC response patching, CA installation, traffic interception, packaging,
  or deployment is implemented in this checkpoint.
