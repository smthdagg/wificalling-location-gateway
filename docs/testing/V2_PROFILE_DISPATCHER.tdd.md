# V2-04 profile dispatcher test evidence

## Scope

The production data path now creates one probe/Geo handler per validated IPv4
device profile while keeping one shared Gateway/WLOC supervisor. The MITM
proxy resolves the source TCP address to exactly one profile target.

## Safety contract

- Every route starts disabled, even when UCI says `enabled=1`.
- A route is enabled only after its isolated redirect is installed and verified.
- Unknown, invalid, disabled, degraded, or target-less routes return no target.
- Manual target clear withdraws that profile until fresh auto evidence returns.
- MAC and IPv6 bindings are rejected by the current IPv4 redirect adapter.

## Verification

| Command | Result |
|---|---|
| `cargo test --all-targets` | Passed; all Rust targets including 6 dispatcher tests |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `tests/scripts/test-profile-redirect.sh` | Passed |
| `tests/scripts/test-profile-status.sh` | Passed |
| `tests/scripts/test-unified-supervisor.sh` | Passed |
| `python3 -m unittest tests.test_v2_ui_contract tests.test_wloc_luci_mode` | Passed |

## Remaining release gates

- Live v2 profile mutation dispatch and LuCI mutations remain Issue #37.
- Real AX6S resource evidence remains Issue #40.
- Update/rollback and migration rehearsal remain Issues #39 and #41.
