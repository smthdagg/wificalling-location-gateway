# WLOC fixture governance contract

This directory is an offline, fail-closed intake boundary. It is not a capture
workspace and does not specify or implement Apple-private protocol fields.

Only two classifications exist:

- `synthetic`: deterministic project-authored generic bytes. These fixtures
  contain no captured traffic and prove only that governance tooling works.
- `authorized-sanitized-capture`: a future, independently approved laboratory
  artifact description. **Phase 0 rejects this classification unconditionally
  with `AuthorizedCaptureGateClosed`.** Self-declared manifest evidence never
  unlocks intake and never counts as a GREEN test result.

No authorized capture is included or approved by this change. The capture gate
must remain closed until a dedicated protocol-aware sanitizer, an external
trusted approval verifier, and independent protocol plus security approvals are
implemented and reviewed. Raw captures remain outside Git at all times. Do not
place PCAP/PCAPNG/HAR files, renamed HAR bodies, request or response dumps, TLS
key logs, archives, PEM/key/profile material, secrets, device/network
identifiers, or precise real coordinates in this repository.

## Manifest and limits

Every candidate uses `fixtures/schema/manifest.schema.json` and records:

- provenance and authorization status/record;
- iOS version (or `not-applicable-synthetic` for synthetic data);
- exactly one of the six approved WLOC hostnames;
- ALPN `h2`, classification, and redactions;
- relative payload path, byte length, and lowercase SHA-256.

The canonical schema bytes are pinned in the zero-dependency validator by
SHA-256. A copied-but-modified or substitute schema is rejected. The validator
manually enforces the synthetic constraints and rejects unknown/duplicate
fields, Unicode/bidirectional filenames, unsafe paths, symlinks, schema depth
over 12, manifests over 64 KiB, and payloads over 1 MiB. Reads use no-follow,
directory-relative file descriptors and `fstat`; errors return stable codes and
do not echo untrusted fields or content. It reads no credentials and performs no
network requests.

The future capture schema requires non-blank creator/date, protocol and security
review attestations (`agent_id`, capabilities, `APPROVE` verdict), clean-room
attestation, off-repository raw retention/deletion evidence, sanitizer
ID/version/PASS result, and explicit `removed`, `verified-absent`, or
`not-applicable` results for secrets, device IDs, network IDs, precise location,
and raw body. These self-declared fields are documentation only in Phase 0.

## Reproducible synthetic path

Generate into a new or existing empty temporary directory, validate, then
delete it. The generator refuses non-empty directories and creates both files
exclusively without overwriting:

```sh
tmp_dir=$(mktemp -d)
python3 scripts/fixtures/generate_synthetic.py \
  --output-dir "$tmp_dir" \
  --fixture-id synthetic-boundary-01 \
  --seed offline-seed-01 \
  --hostname gs-loc.apple.com
python3 scripts/fixtures/fixture_guard.py "$tmp_dir/manifest.json"
```

The same seed and arguments produce byte-identical output and the manifest hash
is checked against the payload. Passing this synthetic path does not authorize
private-protocol parsing, response patching, CA creation, MITM, or real-device
testing.

The only accepted payload is the generator's fixed 61-byte governance format:
the versioned project prefix followed by a SHA-256 seed digest. Unknown binary
formats fail closed even when their manifest hash matches.

## Repository inventory gate

Scan the complete fixture tree before review or CI integration:

```sh
python3 scripts/fixtures/fixture_guard.py --inventory fixtures
```

At the root, only this README, the canonical schema directory, and fixture
directories are allowed. Each fixture directory must contain exactly one
`manifest.json` and its registered `fixture.bin`; only validated synthetic
fixtures pass. Orphans, unknown files, symbolic links, capture extensions,
renamed HAR content, and unregistered payloads fail closed.

## Future authorized intake

An authorized sample would require prior written owner consent for a dedicated
test device, a time-bounded collection approval, legal/license review, a
documented off-repository raw-data retention and deletion process, a separate
protocol-aware sanitizer, external approval verification, and independent
protocol plus security review. Until all of those mechanisms land and receive
new review, the validator must still return `AuthorizedCaptureGateClosed`. If
provenance, authorization, sanitization, or license status cannot be proved,
reject the sample rather than weakening the gate.
