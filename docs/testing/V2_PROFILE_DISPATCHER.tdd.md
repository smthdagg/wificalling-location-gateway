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
- Multi-profile mode never installs the legacy all-device `wloc_service` table;
  only verified profile-scoped tables may intercept.
- Profile-scoped start installs the shared fwmark/local policy route required
  by every profile TPROXY chain, without recreating the legacy table.
- Supervisor, init, and CA/reload cleanup paths remove all profile tables;
  refresh deletes disabled or orphaned tables instead of refreshing them. A
  stale legacy table is removed during profile-mode startup and refresh.
- In supervised mode, the daemon publishes proxy-listener readiness first;
  the supervisor then installs the shared route and signals profile activation.
  Profile redirects are not installed until that signal and readiness return.

## Verification

| Command | Result |
|---|---|
| `cargo test --all-targets` | Passed; all Rust targets including 6 dispatcher tests |
| `cargo clippy --all-targets --all-features -- -D warnings` | Passed |
| `tests/scripts/test-profile-redirect.sh` | Passed |
| `tests/scripts/test-profile-status.sh` | Passed |
| `tests/scripts/test-unified-supervisor.sh` | Passed |
| `python3 -m unittest tests.test_v2_ui_contract tests.test_wloc_luci_mode` | Passed |
| `cargo llvm-cov --workspace --all-targets --locked --fail-under-lines 80` | Passed; 80.74% lines |

The independent review initially returned `REQUEST_CHANGES` for global redirect
ownership, stale profile-table cleanup, and the shared policy route. Those
findings were fixed before handoff and are covered by the profile helper and
unified supervisor shell tests above.

## Remaining release gates

- UCI-backed profile mutation and LuCI Apply/restart are covered by the current
  V2 integration branch and `docs/api/WLOC_SERVICE_API_V2_PROFILES.md`.
- Real AX6S resource, migration, interrupted-update and rollback evidence remain
  required by Issue #41; host tests do not close those gates.
