"""Executable contract for repository-safe WLOC fixtures.

These tests intentionally exercise generic synthetic bytes only. They contain no
Apple-private protocol knowledge and never access the network.
"""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "fixtures" / "schema" / "manifest.schema.json"
GUARD_PATH = ROOT / "scripts" / "fixtures" / "fixture_guard.py"
GENERATOR = ROOT / "scripts" / "fixtures" / "generate_synthetic.py"


def load_guard():
    spec = importlib.util.spec_from_file_location("fixture_guard", GUARD_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("fixture guard cannot be loaded")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_generator():
    spec = importlib.util.spec_from_file_location("generate_synthetic", GENERATOR)
    if spec is None or spec.loader is None:
        raise RuntimeError("synthetic generator cannot be loaded")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FixtureGovernanceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.guard = load_guard()
        self.generator = load_generator()

    def _generate(self, destination: Path, seed: str = "offline-seed-01") -> dict:
        return self.generator.generate(
            destination,
            "synthetic-boundary-01",
            seed,
            "gs-loc.apple.com",
        )

    def test_manifest_schema_requires_security_and_provenance_fields(self) -> None:
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        required = set(schema["required"])
        self.assertTrue(
            {
                "schema_version",
                "fixture",
                "provenance",
                "authorization",
                "ios_version",
                "hostname",
                "alpn",
                "classification",
                "redactions",
            }.issubset(required)
        )
        self.assertEqual(schema["additionalProperties"], False)
        self.assertEqual(
            schema["properties"]["hostname"]["enum"],
            [
                "gs-loc.apple.com",
                "gs-loc-cn.apple.com",
                "gs-loc-corpa.apple.com",
                "gs-loc.apple.com.cn",
                "bluedot.is.autonavi.com",
                "bluedot.is.autonavi.com.gds.alibabadns.com",
            ],
        )
        fixture_required = set(schema["properties"]["fixture"]["required"])
        self.assertTrue({"path", "byte_length", "sha256"}.issubset(fixture_required))

    def test_synthetic_generation_is_offline_deterministic_and_hash_verified(self) -> None:
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            first_dir, second_dir = Path(first), Path(second)
            manifest_a = self._generate(first_dir)
            manifest_b = self._generate(second_dir)
            payload_a = (first_dir / manifest_a["fixture"]["path"]).read_bytes()
            payload_b = (second_dir / manifest_b["fixture"]["path"]).read_bytes()

            self.assertEqual(payload_a, payload_b)
            self.assertEqual(manifest_a, manifest_b)
            self.assertEqual(
                manifest_a["fixture"]["sha256"], hashlib.sha256(payload_a).hexdigest()
            )
            self.assertEqual(manifest_a["classification"], "synthetic")
            self.assertEqual(manifest_a["authorization"]["status"], "not-required-synthetic")
            self.assertEqual(manifest_a["ios_version"], "not-applicable-synthetic")
            self.guard.validate_fixture(first_dir / "manifest.json", SCHEMA)

        self.assertEqual(
            self.generator.build_payload("offline-seed-01"),
            self.generator.build_payload("offline-seed-01"),
        )

    def test_authorized_classification_requires_consistent_approval_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            manifest = self._generate(directory)
            manifest["classification"] = "authorized-sanitized-capture"
            manifest["provenance"]["kind"] = "authorized-lab-capture"
            manifest["authorization"] = {"status": "approved", "record_id": "approval-17"}
            manifest["ios_version"] = "17.6.1"
            manifest["redactions"] = ["device-identifiers", "precise-coordinates"]
            (directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(self.guard.FixtureRejected) as caught:
                self.guard.validate_fixture(directory / "manifest.json", SCHEMA)
            self.assertEqual(caught.exception.code, "AuthorizedCaptureGateClosed")

    def test_validator_rejects_non_exact_hostname_and_alpn(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            manifest = self._generate(directory)
            for field, bad_value in (("hostname", "example.invalid"), ("alpn", "http/1.1")):
                broken = dict(manifest)
                broken[field] = bad_value
                (directory / "manifest.json").write_text(json.dumps(broken), encoding="utf-8")
                with self.subTest(field=field), self.assertRaises(self.guard.FixtureRejected):
                    self.guard.validate_fixture(directory / "manifest.json", SCHEMA)

    def test_sanitizer_rejects_sensitive_identifiers_secrets_and_precise_coordinates(self) -> None:
        prohibited = {
            "bearer token": ("Bearer " + "eyJ" + ".fixture.token").encode(),
            "private key": ("-----BEGIN " + "PRIVATE KEY-----\nnot-a-key").encode(),
            "mac": ("device_mac=" + ":".join(["02", "11", "22", "33", "44", "55"])).encode(),
            "imei": b"imei=" + b"123456789012345",
            "udid": b"udid=" + (b"a" * 40),
            "device id": b"device_id=dedicated-phone-01",
            "precise coordinates": b'{"latitude":0.123456,"longitude":0.654321}',
        }
        for name, payload in prohibited.items():
            with self.subTest(name=name), self.assertRaises(self.guard.FixtureRejected):
                self.guard.inspect_payload(payload, filename="fixture.bin")

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            manifest = self._generate(directory)
            manifest["provenance"]["source_basis"] = "token=" + "fixture-secret-value"
            (directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.validate_fixture(directory / "manifest.json", SCHEMA)

    def test_sanitizer_rejects_raw_capture_formats_and_magic(self) -> None:
        for filename in (
            "traffic.pcap",
            "traffic.pcapng",
            "traffic.har",
            "session.keys",
            "identity.pem",
            "profile.mobileconfig",
            "capture.zip",
        ):
            with self.subTest(filename=filename), self.assertRaises(self.guard.FixtureRejected):
                self.guard.inspect_payload(b"generic", filename=filename)

        with self.assertRaises(self.guard.FixtureRejected):
            self.guard.inspect_payload(bytes.fromhex("d4c3b2a1") + b"generic", filename="fixture.bin")

    def test_size_schema_path_and_hash_limits_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "path-case"
            manifest = self._generate(directory)

            manifest["fixture"]["path"] = "../outside.bin"
            (directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.validate_fixture(directory / "manifest.json", SCHEMA)

            directory = Path(temporary) / "hash-case"
            manifest = self._generate(directory)
            manifest["fixture"]["sha256"] = "0" * 64
            (directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.validate_fixture(directory / "manifest.json", SCHEMA)

            directory = Path(temporary) / "unknown-case"
            manifest = self._generate(directory)
            manifest["unknown"] = "must fail closed"
            (directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.validate_fixture(directory / "manifest.json", SCHEMA)

        with tempfile.TemporaryDirectory() as temporary:
            oversized = Path(temporary) / "fixture.bin"
            oversized.write_bytes(b"x" * (self.guard.MAX_PAYLOAD_BYTES + 1))
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.inspect_file(oversized)

    def test_schema_size_and_duplicate_json_keys_are_bounded(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self._generate(directory)
            oversized_schema = directory / "oversized-schema.json"
            oversized_schema.write_text(
                json.dumps(
                    {
                        "$schema": "https://json-schema.org/draft/2020-12/schema",
                        "padding": "x" * (self.guard.MAX_SCHEMA_BYTES + 1),
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.validate_fixture(directory / "manifest.json", oversized_schema)

            (directory / "manifest.json").write_text(
                '{"schema_version":"wloc-fixture/v1","schema_version":"duplicate"}',
                encoding="utf-8",
            )
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.validate_fixture(directory / "manifest.json", SCHEMA)

    def test_generator_refuses_symlink_output_targets(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            external = directory / "external.bin"
            external.write_bytes(b"must-remain-unchanged")
            output = directory / "output"
            output.mkdir()
            (output / "fixture.bin").symlink_to(external)

            result = subprocess.run(
                [
                    sys.executable,
                    str(GENERATOR),
                    "--output-dir",
                    str(output),
                    "--fixture-id",
                    "synthetic-boundary-01",
                    "--seed",
                    "offline-seed-01",
                    "--hostname",
                    "gs-loc.apple.com",
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
                timeout=5,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(external.read_bytes(), b"must-remain-unchanged")

    def test_validator_rejects_malformed_nested_and_mismatched_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "invalid-utf8"
            manifest = self._generate(directory)

            (directory / "manifest.json").write_bytes(b"\xffnot-utf8")
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.validate_fixture(directory / "manifest.json", SCHEMA)

            directory = Path(temporary) / "nested"
            directory.mkdir()
            nested: object = "too-deep"
            for _ in range(self.guard.MAX_JSON_DEPTH + 2):
                nested = [nested]
            (directory / "manifest.json").write_text(json.dumps(nested), encoding="utf-8")
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.validate_fixture(directory / "manifest.json", SCHEMA)

            directory = Path(temporary) / "length"
            manifest = self._generate(directory)
            manifest["fixture"]["byte_length"] += 1
            (directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.validate_fixture(directory / "manifest.json", SCHEMA)

            invalid_schema = directory / "invalid-schema.json"
            invalid_schema.write_text('{"$schema":"unexpected"}', encoding="utf-8")
            with self.assertRaises(self.guard.FixtureRejected):
                self.guard.validate_fixture(directory / "manifest.json", invalid_schema)

    def test_generator_and_validator_cli_entrypoints_are_bounded(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            generator_args = [
                str(GENERATOR),
                "--output-dir",
                str(directory),
                "--fixture-id",
                "synthetic-cli-01",
                "--seed",
                "cli-seed",
                "--hostname",
                "gs-loc-cn.apple.com",
            ]
            with patch.object(sys, "argv", generator_args):
                self.assertEqual(self.generator.main(), 0)

            guard_args = [
                str(GUARD_PATH),
                str(directory / "manifest.json"),
                "--schema",
                str(SCHEMA),
            ]
            output = io.StringIO()
            with patch.object(sys, "argv", guard_args), redirect_stdout(output):
                self.assertEqual(self.guard.main(), 0)
            self.assertIn("fixture accepted: synthetic-cli-01", output.getvalue())

            with self.assertRaises(ValueError):
                self.generator.generate(directory, "BAD ID", "seed", "gs-loc.apple.com")
            with self.assertRaises(ValueError):
                self.generator.generate(directory, "safe-id", "", "gs-loc.apple.com")


if __name__ == "__main__":
    unittest.main()
