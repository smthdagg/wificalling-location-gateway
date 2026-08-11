# Phase 0 offline-scaffold review

Date: 2026-08-11

## Reviewed documents

- `docs/adr/0001-license-boundary.md`
- `fixtures/wloc/README.md`
- `docs/security/WLOC_THREAT_MODEL.md`

## Review history

The first independent consistency review blocked progression on three P1 issues:

1. GitHub Issue owned paths differed from the canonical document paths.
2. Fixture category names differed between the ADR and governance contract.
3. The fixture manifest contract did not explicitly require ALPN and redaction metadata.

The fixes added non-normative compatibility pointers at the Issue-owned paths,
standardized fixture kinds to `synthetic` and `authorized-sanitized-capture`,
limited `public-document-observation` to protocol-note review records, added
ALPN and `redactions[]`, and placed the same next-step restriction in all three
canonical documents.

The consistency re-review found the fixes complete. A separate fresh-context
security Reviewer reported no remaining P0, P1, or P2 finding for the approved
scope.

## Approved scope

- Go 1.23 module metadata compatible with the OpenWrt 24.10 toolchain baseline.
- Offline manifest validator and CI scaffolding.
- Generic protocol safety-contract types and tests that use no private protocol knowledge.

## Explicitly not approved

- Apple private field numbers, field semantics, or source-derived schemas.
- Real or sanitized capture bytes; no fixture has yet passed the governance workflow.
- WLOC parsing, response patching, CA generation, TLS interception, MITM, router redirect, or live Apple traffic.
- A production deployment, emergency-call claim, or repository license grant.

This is a documentation and offline-scaffolding approval. Every later hard gate
in the threat model remains in force and requires its own implementation and
test evidence.
