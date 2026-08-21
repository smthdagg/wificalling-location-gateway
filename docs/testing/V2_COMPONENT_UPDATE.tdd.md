# V2-07 component update TDD evidence

## Source plan

Journeys were derived from GitHub Issue #39: validate a package before mutation,
preserve configuration, reject low storage, support downgrade only when
explicitly authorized, and recover automatically after failed activation or
interruption.

## RED/GREEN checkpoints

- `90288e7 test: define component update rollback contract` — RED: the new
  executable test ran and failed because the update helper did not exist.
- `a8d3e70 fix: add transactional component update and rollback` — GREEN: the
  same test passed for successful update, health rollback, interrupted recovery,
  and low-space preflight.

## Test specification

| Guarantee | Test | Result |
|---|---|---|
| A valid compatible IPK is installed without removing first | `tests/scripts/test-component-update.sh` | PASS |
| Configuration survives package overwrite | `tests/scripts/test-component-update.sh` | PASS |
| Wrong architecture and unauthorized downgrade are rejected before opkg | `tests/scripts/test-component-update.sh` | PASS |
| Explicit downgrade is supported | `tests/scripts/test-component-update.sh` | PASS |
| Failed health activation restores the known-good package and config | `tests/scripts/test-component-update.sh` | PASS |
| Interrupted transaction is recoverable | `tests/scripts/test-component-update.sh` | PASS |
| Low storage fails before package mutation | `tests/scripts/test-component-update.sh` | PASS |
| RPC, ACL, UI, package paths, and compatibility metadata remain aligned | `tests/test_v2_diagnostics_contract.py` | PASS |

## Coverage and known gaps

The shell transaction contract is deterministic and runs through the full
repository gate. Real AX6S power-loss timing, opkg database recovery after a
hard power cut, and an on-device flash-full fault still require Issue #40
hardware evidence. The helper deliberately requires a known-good rollback IPK;
first installation is outside the update transaction and must establish that
baseline before upgrades are enabled.
