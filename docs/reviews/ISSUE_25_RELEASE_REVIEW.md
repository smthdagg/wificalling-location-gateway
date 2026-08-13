# Issue 25 release review

- Reviewer: `release_1_0_review`
- Capability: independent release/code review
- Initial verdict: `REQUEST_CHANGES`
- Findings: Docker artifact selection was not bound to the manifest, and the
  exact AX6S upgrade/configuration-preservation evidence was not recorded.
- Remediation: Docker selects only the exactly three `SHA256SUMS` entries and
  rejects unlisted matching packages. The exact final AX6S asset preserved
  both UCI hashes and passed service, LuCI, and mode-switch validation.
- Re-review found one P2 builder/verifier mismatch: the builder wrote `./`
  paths while the verifier correctly requires basenames. The builder now emits
  basename-only entries and its regression test forbids the incompatible form.
- Final verdict: `APPROVE` after the reviewer reproduced the corrected builder
  output and passed the four-environment Docker matrix with no P0/P1/P2.
