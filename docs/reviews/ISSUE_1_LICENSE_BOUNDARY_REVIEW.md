# Issue #1 license-boundary review and provenance record

- Record ID: `issue-1-license-boundary-review-v1`
- Date: 2026-08-11
- Scope: license status, clean-room boundary, contributor separation, fixture
  provenance requirements, and Phase 0 authorization limits
- Canonical decision: [`docs/adr/0001-license-boundary.md`](../adr/0001-license-boundary.md)
- Issue-owned compatibility path:
  [`docs/security/adr-001-license-boundary.md`](../security/adr-001-license-boundary.md)
- Current disposition: **APPROVED by independent protocol and security reviewers**

This record uses stable artifact IDs, repository-relative paths, content
digests, and public document URLs. Its provenance does not depend on a branch
name, pull-request number, or mutable project commit hash. If a reviewed
artifact's digest changes, the corresponding reviewer attestation is stale and
must be repeated.

## Commit-independent artifact provenance

| Artifact ID | Artifact or input | Origin and authorization | SHA-256 or stable locator | Use in this decision |
|---|---|---|---|---|
| `LIC-ADR-001` | `docs/adr/0001-license-boundary.md` | Original project policy text, remediated under Issue #1 by `codex-license-close`; no reference implementation source used | `aac63c6fc7cf54da3658fda4e2e62fcabfb17868234f8409e48ea98da56b874c` | Normative decision; reviewers attest against this exact content |
| `LIC-DRAFT-001` | Coordinator workspace draft `docs/adr/0001-license-boundary.md` | Project-authored Phase 0 draft supplied as an allowed policy input; not reference source, code, fixture, or protocol implementation | `88aa925600047a7e8895c01fdbbce9e60d2a01204abb9614e6f5fe294d19bccd` | Starting policy language and previously identified boundary rules |
| `LIC-REVIEW-LEGACY-001` | Coordinator workspace `docs/reviews/PHASE0_OFFLINE_SCAFFOLD_REVIEW.md` | Project-authored summary supplied for read-only comparison | `b891afe1ce75076e3a4df4ba81a821ab794e1b8a9f4e877a5db6c71713e917eb` | Historical findings only; anonymous claims in it are not treated as durable approval |
| `PROJECT-PLAN-001` | `DEVELOPMENT_TEST_PLAN.md` | Repository source of truth | `64939d222f9f6789b96a1d697e75bb76f942a69978b9f28d97bbb7792319dd13` | AGPL/MIT incompatibility risk, Phase 0 alternatives, and exit limits |
| `PROJECT-CONTRACT-001` | `AGENTS.md` | Repository source of truth | `81a1dc0ef9edab2033b33f7a1bb4296ec097f214b8fec07f482c066b504c6ce6` | Ownership, dual-review, prohibited data, and implementation hard gates |
| `PROJECT-SECURITY-001` | `SECURITY.md` | Repository source of truth | `06b3135d16b12e63750732de4f2b617e6962613603a7fbd3c8a3f08c787eacf2` | Fixture/data restrictions and independent-review requirement |
| `ISSUE-1` | GitHub Issue #1, “ADR: freeze license and clean-room protocol boundary” | Repository owner-authored durable work specification, read-only access on 2026-08-11 | `https://github.com/smthdagg/wificalling-location-gateway/issues/1` | Owned paths, acceptance criteria, non-goals, and required capabilities |
| `AGPL-3.0` | GNU Affero General Public License, version 3 | Public license text; factual license-boundary research only | `https://www.gnu.org/licenses/agpl-3.0.html` | Network-source obligation context; no implementation content |
| `GNU-FAQ` | GNU license FAQ and compatibility guidance | Public license guidance; factual license-boundary research only | `https://www.gnu.org/licenses/gpl-faq.html` and `https://www.gnu.org/licenses/license-compatibility.html` | Compatibility context; not legal advice |
| `MIT-OSI` | OSI MIT License page | Public license text | `https://opensource.org/license/mit` | MIT notice and permission context |
| `REFERENCE-LICENSE-ONLY` | `ios-location-spoofer` license file at evidence revision `b72d6f67efb2b457647ae05e3e20ae3f3f6f0262` | License-identification evidence only; source tree, code, tests, fixtures, docs, diffs, and structure are prohibited | `https://github.com/mekos2772/ios-location-spoofer/blob/b72d6f67efb2b457647ae05e3e20ae3f3f6f0262/LICENSE` | Establishes declared AGPL-3.0 status only |

The remediation Agent did not open the reference repository or its license URL
during this task. The public-license facts above were inherited from the
project-authored Phase 0 draft. A final reviewer may independently verify only
the license notice and public legal text; no reviewer may feed reference source
content into implementation context.

## Contributor/Agent provenance attestation

| Field | Attestation |
|---|---|
| Agent ID | `codex-license-close` |
| Capability tags | `protocol`, `security`, `docs` |
| Issue and owned scope | Issue #1; canonical ADR, Issue-owned compatibility pointer, README license notice, and this dedicated review/provenance record |
| Role | Policy remediation author; not a final independent approver |
| Reference-source access | No AGPL reference source, tests, fixtures, commits, diffs, screenshots, repository structure, binaries, or source-derived protocol descriptions were inspected for this task |
| AI prompt/context attestation | No prohibited reference material, source excerpts, fixture bytes, raw captures, precise location, device identifier, credential, CA key, or production traffic was supplied to an AI prompt or generated artifact |
| Permitted inputs used | The artifacts enumerated in the provenance table; GitHub Issue #1 was read through the GitHub CLI in read-only mode |
| Prior exposure disclosure | No prior reference-source access was disclosed or observed in the task context; if later contradicted, this attestation is invalid and the affected implementation role must be reassigned |
| Credential handling | No credentials were copied, printed, recorded, or transferred |

This attestation covers documentation remediation only. It does not qualify the
Agent to implement or review WLOC protocol behavior.

## Audit history

| Reviewer Agent ID | Capability attestation | Independence and scope | Result |
|---|---|---|---|
| `wloc_phase0_gate_audit` | `security`, `docs` | Read-only Phase 0 audit; did not author the remediation | P1: the prior ADR was substantively close, but lacked durable reviewer Agent IDs/capability attestations and commit-independent provenance. Required this remediation. This is a finding, **not final approval**. |
| `license_protocol_review` | `protocol`, `docs`; reviewed `aac63c6fc7cf54da3658fda4e2e62fcabfb17868234f8409e48ea98da56b874c` | Independent read-only review; no reference implementation source inspected; verified clean-room inputs, role separation, provenance, and absence of private protocol design | **APPROVE**, 2026-08-11; no P0/P1/P2, one non-blocking P3 wording note |
| `license_security_review` | `security`, `docs`; reviewed `aac63c6fc7cf54da3658fda4e2e62fcabfb17868234f8409e48ea98da56b874c` | Independent read-only security/license-boundary engineering review; verified secret/private-data exclusions, all-rights-reserved wording, quarantine, and non-authorization scope | **APPROVE**, 2026-08-11; no P0/P1/P2/P3 |

The legacy Phase 0 review summary reported an earlier consistency review and a
fresh-context security review, but it did not name non-secret reviewer Agent
IDs or capability attestations. Those anonymous statements are retained only
as history and do not satisfy Issue #1's durable dual-role review gate.

## Required final reviewer attestations

The protocol reviewer must attest that:

- allowed and prohibited source inputs are operationally unambiguous;
- observer and clean-room implementer roles cannot overlap for the same
  protocol behavior;
- fixture and protocol-note provenance rules do not authorize reference-source
  copying or unreviewed captures;
- the ADR contains no Apple private field knowledge, parser/patch design, or
  source-derived protocol schema;
- only the license-boundary portion of Phase 0 is affected.

The security reviewer must attest that:

- repository status remains all rights reserved until a `LICENSE` is granted;
- no ADR wording claims that process/API separation removes AGPL obligations;
- AI prompts and generated artifacts are explicitly within the clean-room
  prohibition;
- raw traffic, identifiers, precise location, credentials, and CA keys remain
  prohibited;
- no parser, patch, CA, MITM, redirect, or real-device authorization is
  implied;
- the provenance table and author attestation are complete and internally
  consistent.

Both final reviewers supplied their real non-secret Agent IDs, capability tags,
exact reviewed `LIC-ADR-001` digest, date, findings, and explicit approval.
Approval becomes stale if the canonical ADR digest changes.

## Current approved and blocked scope

The dual-role review requirement is satisfied for the exact ADR digest recorded
above. Changing the canonical ADR requires a new protocol and security review.

Even after dual approval, this record resolves only the license-boundary part
of Phase 0. It permits no WLOC parser, response patcher, CA, TLS interception,
MITM, router redirect, real fixture bytes, or live-device traffic. Subsequent
work remains bound by the separate authorized-fixture and threat-model gates.
