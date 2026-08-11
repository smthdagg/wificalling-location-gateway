"""Regression tests for the Phase 0 protocol and security review findings."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "fixtures" / "schema" / "manifest.schema.json"
GUARD_PATH = ROOT / "scripts" / "fixtures" / "fixture_guard.py"
GENERATOR_PATH = ROOT / "scripts" / "fixtures" / "generate_synthetic.py"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError("test module cannot be loaded")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FixtureSecurityReviewTests(unittest.TestCase):
    def setUp(self) -> None:
        self.guard = load_module("fixture_guard_security", GUARD_PATH)
        self.generator = load_module("generate_synthetic_security", GENERATOR_PATH)

    def _generate(self, directory: Path) -> dict:
        return self.generator.generate(
            directory,
            "synthetic-security-01",
            "review-seed",
            "gs-loc.apple.com",
        )

    def _assert_rejected(self, expected_code: str, operation) -> None:
        with self.assertRaises(self.guard.FixtureRejected) as caught:
            operation()
        self.assertEqual(caught.exception.code, expected_code)
        self.assertEqual(str(caught.exception), expected_code)

    def test_canonical_schema_digest_and_manual_constants_are_pinned(self) -> None:
        schema_bytes = SCHEMA.read_bytes()
        schema = json.loads(schema_bytes)
        self.assertEqual(hashlib.sha256(schema_bytes).hexdigest(), self.guard.TRUSTED_SCHEMA_SHA256)
        self.assertEqual(
            tuple(schema["properties"]["hostname"]["enum"]), self.guard.ALLOWED_HOSTNAMES
        )
        self.assertEqual(
            schema["properties"]["fixture"]["properties"]["byte_length"]["maximum"],
            self.guard.MAX_PAYLOAD_BYTES,
        )
        self.assertEqual(
            schema["properties"]["classification"]["enum"],
            ["synthetic", "authorized-sanitized-capture"],
        )

    def test_authorized_capture_is_unconditionally_gate_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            manifest = self._generate(directory)
            manifest["classification"] = "authorized-sanitized-capture"
            manifest["authorization"] = {"status": "approved", "record_id": "self-declared"}
            (directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            self._assert_rejected(
                "AuthorizedCaptureGateClosed",
                lambda: self.guard.validate_fixture(directory / "manifest.json", SCHEMA),
            )

    def test_schema_describes_future_external_authorization_evidence(self) -> None:
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        authorization = schema["$defs"]["captureAuthorization"]
        self.assertTrue(
            {
                "creator",
                "created_at",
                "protocol_review",
                "security_review",
                "clean_room_attestation",
                "raw_retention",
                "sanitizer",
                "redaction_evidence",
            }.issubset(set(authorization["required"]))
        )
        reviewer_required = set(schema["$defs"]["reviewAttestation"]["required"])
        self.assertEqual(reviewer_required, {"agent_id", "capabilities", "verdict"})
        evidence_required = set(schema["$defs"]["redactionEvidence"]["required"])
        self.assertEqual(
            evidence_required,
            {"secrets", "device_ids", "network_ids", "precise_location", "raw_body"},
        )

    def test_high_confidence_sensitive_encodings_are_rejected(self) -> None:
        candidates = {
            "github pat": ("gh" + "p_" + "A" * 40).encode(),
            "github fine-grained pat": ("github" + "_pat_" + "A" * 40).encode(),
            "openai": ("s" + "k-proj-" + "A" * 32).encode(),
            "aws": ("AK" + "IA" + "A" * 16).encode(),
            "jwt": ("eyJ" + "hbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.signature123").encode(),
            "bare mac": ":".join(["02", "11", "22", "33", "44", "55"]).encode(),
            "bare hyphen mac": "-".join(["02", "11", "22", "33", "44", "55"]).encode(),
            "bare imei": b"0" * 15,
            "bare udid": b"a" * 40,
            "bare modern udid": ("00008101-" + "a" * 16).encode(),
            "coordinate array": b"[0.123456,0.654321]",
            "renamed har": b'{"log":{"version":"1.2","entries":[]}}',
        }
        for name, payload in candidates.items():
            with self.subTest(name=name):
                self._assert_rejected(
                    "SensitiveContent",
                    lambda payload=payload: self.guard.inspect_payload(payload, "fixture.bin"),
                )

        utf16_secret = ("token=" + "encoded-secret-value").encode("utf-16-le")
        self._assert_rejected(
            "SensitiveContent",
            lambda: self.guard.inspect_payload(utf16_secret, "fixture.bin"),
        )

    def test_only_fixed_synthetic_payload_format_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            manifest = self._generate(directory)
            payload = directory / "fixture.bin"
            payload.write_bytes(b"unknown-binary-format\x00\x01")
            changed = payload.read_bytes()
            manifest["fixture"]["byte_length"] = len(changed)
            manifest["fixture"]["sha256"] = hashlib.sha256(changed).hexdigest()
            (directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            self._assert_rejected(
                "UnsupportedSyntheticPayload",
                lambda: self.guard.validate_fixture(directory / "manifest.json", SCHEMA),
            )

    def test_fake_or_modified_schema_and_bidi_paths_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            manifest = self._generate(directory)
            fake_schema = directory / "schema.json"
            fake_schema.write_text(SCHEMA.read_text(encoding="utf-8") + " ", encoding="utf-8")
            self._assert_rejected(
                "UntrustedSchema",
                lambda: self.guard.validate_fixture(directory / "manifest.json", fake_schema),
            )

            manifest["fixture"]["path"] = "safe\u202egnp.exe"
            (directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            self._assert_rejected(
                "UnsafePath",
                lambda: self.guard.validate_fixture(directory / "manifest.json", SCHEMA),
            )

    def test_redactions_non_string_is_a_controlled_rejection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            manifest = self._generate(directory)
            manifest["redactions"] = [{}]
            (directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            self._assert_rejected(
                "InvalidRedactions",
                lambda: self.guard.validate_fixture(directory / "manifest.json", SCHEMA),
            )

    def test_repository_inventory_rejects_orphans_and_renamed_captures(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            inventory = Path(temporary)
            (inventory / "schema").mkdir()
            (inventory / "README.md").write_text("fixture inventory\n", encoding="utf-8")
            shutil.copyfile(SCHEMA, inventory / "schema" / "manifest.schema.json")
            self._generate(inventory / "synthetic-security-01")
            self.guard.validate_inventory(inventory, inventory / "schema" / "manifest.schema.json")

            for name, content in (
                ("orphan.bin", b"orphan"),
                ("renamed.bin", b'{"log":{"version":"1.2","entries":[]}}'),
                ("capture.pcap", bytes.fromhex("d4c3b2a1") + b"capture"),
            ):
                candidate = inventory / name
                candidate.write_bytes(content)
                with self.subTest(name=name):
                    self._assert_rejected(
                        "InventoryViolation",
                        lambda: self.guard.validate_inventory(
                            inventory, inventory / "schema" / "manifest.schema.json"
                        ),
                    )
                candidate.unlink()

    def test_safe_open_rejects_symlink_and_generator_never_overwrites(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            external = root / "external"
            external.write_bytes(b"unchanged")
            output = root / "output"
            output.mkdir()
            (output / "existing.txt").write_text("occupied", encoding="utf-8")
            with self.assertRaises(ValueError):
                self._generate(output)
            self.assertEqual((output / "existing.txt").read_text(encoding="utf-8"), "occupied")

            empty = root / "empty"
            empty.mkdir()
            (empty / "fixture.bin").symlink_to(external)
            with self.assertRaises(ValueError):
                self._generate(empty)
            self.assertEqual(external.read_bytes(), b"unchanged")

            broken = root / "broken-output"
            broken.symlink_to(root / "missing-target", target_is_directory=True)
            with self.assertRaises(ValueError):
                self._generate(broken)

            generated = root / "generated"
            self._generate(generated)
            replacement = root / "replacement"
            replacement.write_bytes((generated / "fixture.bin").read_bytes())
            (generated / "fixture.bin").unlink()
            (generated / "fixture.bin").symlink_to(replacement)
            self._assert_rejected(
                "UnsafeFile",
                lambda: self.guard.validate_fixture(generated / "manifest.json", SCHEMA),
            )


if __name__ == "__main__":
    unittest.main()
