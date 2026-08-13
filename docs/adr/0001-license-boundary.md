# ADR-0001: License and clean-room implementation boundary

> Repository license update (2026-08-13): the repository owner subsequently
> granted the original project work under the MIT License by committing the
> root [`LICENSE`](../../LICENSE). The historical wording below records the
> license state when this ADR was accepted; this ADR itself remains a boundary
> decision and is not the instrument that grants the license. The clean-room,
> provenance, quarantine, and third-party-license requirements remain in force.

- Status: Proposed for offline-scaffolding acceptance; independent protocol and security sign-off pending
- Date: 2026-08-11
- Decision owners: protocol and security roles
- Phase: 0
- Supersedes: none
- Issue-owned pointer: [Issue #1 compatibility path](../security/adr-001-license-boundary.md)
- Review and provenance record: [Issue #1 license-boundary review](../reviews/ISSUE_1_LICENSE_BOUNDARY_REVIEW.md)

## Notice

This record is an engineering and contribution policy, not legal advice or a
guarantee of non-infringement. Copyright scope and license obligations depend
on facts and jurisdiction. Material uncertainty must be escalated to the
copyright holders or qualified counsel before release.

## Context

`wificalling-location-gateway` is an isolated component intended to integrate
with Wi-Fi Calling Gateway through documented configuration and data
contracts. Wi-Fi Calling Gateway is licensed under MIT. The protocol reference
identified by the development plan, `mekos2772/ios-location-spoofer`, declares
AGPL-3.0.

The MIT license does not remove conditions attached to code received under
another license. AGPLv3 section 13 also applies network-source obligations to
covered modified versions that support remote network interaction. Repository,
process, or API separation alone does not prove independence and does not
relicense copied or derived code. This ADR therefore establishes both an
integration boundary and a provenance-controlled implementation process.

## Decision

This project selects the **independent clean-room implementation route**.

The WLOC parser, response patcher, fixtures, protocol notes, tests, and related
interfaces must be created without copying, adapting, translating, or using
the structure of the AGPL reference implementation. The reference repository
must not be a source dependency, build dependency, test dependency, submodule,
vendored artifact, or prompt/context input for those components.

The intended publication outcome is an original implementation that may later
use a permissive, Gateway-compatible license. This ADR does **not** grant a
license. Until repository ownership and notice review results in a committed
`LICENSE`, the repository remains **all rights reserved by default** and must
not be described as MIT-licensed.

Wi-Fi Calling Gateway remains a separately versioned MIT project. Integration
is limited to documented configuration, process-control, and data contracts.
No Gateway source may be copied or vendored merely for convenience. Any future
reuse must preserve the applicable MIT notice and be entered in the provenance
record before use.

## Permitted clean-room inputs

An implementer may use only inputs whose origin and authorization are recorded:

1. Public standards, vendor documentation, and library API documentation with
   stable URL, title, version or access date, and the fact relied upon.
2. Original facts learned from authorized black-box interoperability tests,
   without copied prose, diagrams, identifiers, control flow, data structures,
   or test cases from the reference source.
3. Synthetic fixtures created from an independently written schema or test
   requirement.
4. Explicitly authorized device captures that have passed the separate fixture
   governance, sanitization, and security reviews.
5. Protocol notes written from allowed behavioral observations and containing
   no source excerpts, source-derived pseudocode, original comments,
   distinctive names, file layout, or implementation suggestions.
6. Original contributions whose authors attest to provenance and disclose
   relevant prior access.
7. MIT Gateway documentation and, only after explicit provenance review, MIT
   Gateway code used in compliance with its notice and permission terms.

A public URL does not by itself authorize copying expressive content. Each
input remains subject to its own license and terms.

## Prohibited clean-room inputs

The following must not enter implementation branches, Issues, pull requests,
fixtures, protocol notes, AI prompts, generated artifacts, or review comments:

- source code, patches, diffs, binaries, source maps, generated output, tests,
  fixtures, comments, documentation text, source screenshots, or repository
  structure copied from the AGPL reference implementation;
- translations, paraphrased pseudocode, distinctive algorithms, names,
  constants, tables, schemas, or test vectors derived from inspecting it;
- decompiled or disassembled reference artifacts, or reconstruction-tool
  output;
- third-party summaries that reproduce or closely transform protected source;
- raw production traffic, unauthorized captures, device identifiers, BSSID or
  cell observations, credentials, CA private keys, precise user location, or
  fixtures without recorded capture authority and sanitization;
- claims that repository/process separation, RPC, dynamic linking, or an API
  boundary automatically removes AGPL or copyright obligations.

Copying AGPL-covered material is not made acceptable by adding an MIT header.
Intentional reuse suspends this decision and invokes the exception process.

## Contributor and Agent separation

The workflow has two mutually exclusive protocol roles.

### Reference-side observer

A reference-side observer may inspect the AGPL repository only under a
separately approved compatibility investigation. They may report factual,
externally observable behavior, but must not write or review the clean-room
WLOC parser, patcher, protocol fixtures, or protocol-specific tests. Their
deliverable requires protocol and security review for source leakage.

### Clean-room implementer

A clean-room implementer must not inspect the AGPL source, tests, fixtures,
commits, pull requests, or source-derived descriptions for this work. Before
accepting a protocol Issue, they must record:

- a non-secret contributor or Agent ID;
- the Issue and owned paths;
- approved specifications and fixture artifact IDs used;
- prior access to the reference source, if any, with enough detail for
  reassignment review;
- an attestation that no prohibited input was used or supplied to an AI tool.

Prior exposure is not misconduct, but affected protocol work must be reassigned
when independent reviewers cannot establish a credible clean-room path. AI
Agents are contributors under this policy. Their context may contain only
approved specifications and sanitized fixtures. No person or Agent may be both
reference-side observer and clean-room implementer for the same behavior.

## Fixture and protocol-note provenance

Every committed fixture and protocol note must have an adjacent manifest or
review record containing:

- stable artifact ID and cryptographic digest;
- kind: `synthetic` or `authorized-sanitized-capture` for fixtures;
  protocol-note records may additionally use
  `public-document-observation`, which is never a fixture kind;
- creator or observer ID and creation date;
- capture authority or synthetic generation method;
- source title, version, stable URL, and access date for public documentation;
- sanitization actions and confirmation that prohibited sensitive fields are
  absent;
- named protocol and security reviewer Agent IDs with capability attestations;
- a statement that the artifact was not copied or derived from AGPL source;
- retention and deletion rules for uncommitted raw material.

Raw authorized captures remain outside Git and shared Agent context. Only the
minimum approved sanitized fixture required for a named test may be committed.
Missing, ambiguous, or disputed provenance quarantines the artifact and blocks
dependent work until reviewers approve a replacement.

## Review and enforcement

Before protocol implementation starts, protocol and security reviewers must
confirm all of the following in a commit-independent review record:

- this ADR is accepted and repository license status is stated accurately;
- the fixture governance contract is accepted;
- the protocol specification identifies only allowed inputs;
- every initial fixture has complete provenance metadata;
- observer and implementer roles remain separated;
- dependency and repository scans find no reference package, copied file,
  suspicious source phrase, or AGPL artifact;
- the contributor/Agent provenance attestation is complete.

Automated similarity and dependency scans are supporting controls, not proof
of independent creation. Human protocol and security review remains required.

## Exceptions and future changes

There is no informal exception. A request to reuse AGPL material, relax role
separation, change the intended license, or combine the component with another
work must:

1. stop affected implementation and quarantine the proposed material;
2. open an Issue naming the exact material, copyright holder, license version,
   deployment model, and affected paths;
3. obtain written permission or a reviewed AGPL-3.0 compliance plan, including
   notices, Corresponding Source, build/install materials where applicable,
   and the section 13 network source offer;
4. receive protocol, security, and repository-owner approval, plus qualified
   legal review where uncertainty remains;
5. supersede this ADR and update `LICENSE`, notices, packaging, CI policy, and
   release procedures before using the material.

A future AGPL decision applies prospectively and cannot legitimize undocumented
provenance retroactively.

## Consequences

This decision preserves operational and provenance separation from the stable
Gateway, keeps a permissive-license option for original work, and makes fixture
authorization auditable. It also requires role separation, dual review, and
additional time; clean-room controls reduce risk but are not a legal safe
harbor.

## Phase 0 exit effect

Acceptance of this ADR resolves only the license-boundary portion of Phase 0.
It does not authorize WLOC parsing or response patching until the authorized
fixture contract and threat model are also accepted. It never authorizes real
device interception by itself.

After all three Phase 0 documents are accepted, the next allowed scope is
limited to module metadata, an offline manifest validator, CI scaffolding, and
generic protocol safety-contract tests. That scope must contain no Apple
private field numbers or semantics, real capture bytes, response-patch logic,
CA generation, MITM, or live traffic.

## Authoritative references checked

- [GNU Affero General Public License v3](https://www.gnu.org/licenses/agpl-3.0.html)
- [GNU license FAQ](https://www.gnu.org/licenses/gpl-faq.html)
- [GNU license compatibility guidance](https://www.gnu.org/licenses/license-compatibility.html)
- [Open Source Initiative: MIT License](https://opensource.org/license/mit)
- [`mekos2772/ios-location-spoofer` repository](https://github.com/mekos2772/ios-location-spoofer)
- [`ios-location-spoofer` AGPL-3.0 license at evidence commit `b72d6f67efb2b457647ae05e3e20ae3f3f6f0262`](https://github.com/mekos2772/ios-location-spoofer/blob/b72d6f67efb2b457647ae05e3e20ae3f3f6f0262/LICENSE)
- [Wi-Fi Calling Gateway repository](https://github.com/smthdagg/luci-app-wificalling-gateway) (private source; its local `LICENSE` was reviewed as MIT)
