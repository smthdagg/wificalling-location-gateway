# Agent handoff: Issue 31

## Identity and scope

- Source agent ID: codex-v2-lead
- Capabilities used: rust,openwrt,test
- Branch: codex/issue-31-v2-device-profiles-codex-v2-lead-20260821044654-a86aad99
- Checkpoint parent: 6b0a744
- Updated at (UTC): 2026-08-21T05:50:00Z
- Credentials included: no

## Objective

Implement the bounded v2 device-profile model, legacy singleton projection,
explicit UCI profile parsing, and additive v2 profile API decoding without
changing the current runtime interception path.

## Completed

- Added `DeviceProfile`, `ProfileModel`, node selection and location mode
  fields, validation, AX6S-oriented resource ceilings, redacted status, and
  transactional replacement.
- Added deterministic v1-to-singleton migration and explicit `config device`
  UCI sections with duplicate, address, mode, coordinate, and required-device
  validation.
- Added v2 profile request decoding and bounded v2 result envelopes while
  preserving the frozen v1 API.
- Added profile tests, API tests, UCI example documentation, and the v2 profile
  contract document.
- Runtime now consumes an explicit single profile, rejects unsupported MAC and
  multiple-profile runtime selection, and fail-closes invalid existing UCI.
- Profile-bound probes require a matching Gateway UCI device policy and reject
  route-only or stale sing-box rule fallback.

## Verification

- `cargo test --all-targets`: passed (64 unit tests plus all integration suites).
- `cargo clippy --all-targets -- -D warnings`: passed.
- `./scripts/ci/verify.sh`: passed.
- Python suite: 69 passed.
- Rust coverage: 80.96% total.
- Dependency advisories, secret scan, OpenWrt package, AX6S resource, and
  release gates: passed.
- Relevant commits: `7542dd9`, `f38def9`, `a9ad40a`, `4663edb`, `05d1b42`,
  `6b0a744`, `0061686`.

## Failed attempts

- No product-code verification failure occurred. The repository commit hook
  reports that `lefthook` is unavailable in the local PATH; the required CI
  verification script passed independently.

## Next executable steps

1. Complete independent review of the implementation and binding fallback fix.
2. Merge the branch through the Issue #31 pull request after review.
3. Start V2-02 for the unified procd supervisor and runtime lifecycle.

## Capabilities required for the next Agent

- rust
- openwrt
- test
- security

## Security and privacy notes

- No credentials, private keys, raw production traffic, device identifiers, or
  precise location data are included.
- Explicit v2 profiles require an assigned local IPv4/IPv6 or MAC address, but
  redacted status exposes only configured/not-configured booleans.
- Legacy singleton migration remains compatible with an empty assigned device;
  it does not create a multi-device routing policy.
- Runtime dispatch, nftables routing, procd supervision, LuCI, logs, and update
  operations remain out of scope for this Issue.
- The route-only binding regression is covered by
  `required_device_binding_rejects_route_only_match`.
