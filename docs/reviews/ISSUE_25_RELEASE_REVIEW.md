# Issue 25 release review

- Reviewer: `release_1_0_review`
- Capability: independent release/code review
- Initial verdict: `REQUEST_CHANGES`
- Findings: Docker artifact selection was not bound to the manifest, and the
  exact AX6S upgrade/configuration-preservation evidence was not recorded.
- Remediation: Docker selects only the exactly three `SHA256SUMS` entries and
  rejects unlisted matching packages. The exact final AX6S asset preserved
  both UCI hashes and passed service, LuCI, and mode-switch validation.
- Final verdict: pending re-review.
