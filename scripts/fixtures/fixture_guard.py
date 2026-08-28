#!/usr/bin/env python3
"""Fail-closed validation for repository fixture candidates.

Only the project-generated synthetic governance format can pass. Authorized
capture metadata is descriptive future schema surface and cannot open the gate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
from pathlib import Path
from typing import Any


MAX_MANIFEST_BYTES = 64 * 1024
MAX_SCHEMA_BYTES = 128 * 1024
MAX_PAYLOAD_BYTES = 1024 * 1024
MAX_JSON_DEPTH = 12
ALLOWED_HOSTNAMES = (
    "gs-loc.apple.com",
    "gs-loc-cn.apple.com",
    "gs-loc-corpa.apple.com",
    "gs-loc.apple.com.cn",
    "bluedot.is.autonavi.com",
    "bluedot.is.autonavi.com.gds.alibabadns.com",
)
TRUSTED_SCHEMA_SHA256 = "b2905e94cf1ee4cb5f19588605de2e2253d7f51d2f3fa3a49d2ff687d46472c3"
SYNTHETIC_PREFIX = b"WLG-SYNTHETIC-GOVERNANCE-V1\x00"
SYNTHETIC_PAYLOAD_BYTES = len(SYNTHETIC_PREFIX) + 32
SYNTHETIC_GENERATOR = "scripts/fixtures/generate_synthetic.py"
SYNTHETIC_GENERATOR_VERSION = "1"
SYNTHETIC_SOURCE_BASIS = (
    "project-authored generic bytes; no capture or private protocol source"
)
ALLOWED_REDACTIONS = {
    "credentials",
    "device-identifiers",
    "network-identifiers",
    "precise-coordinates",
    "request-metadata",
    "unrelated-payload",
}
FORBIDDEN_SUFFIXES = {
    ".7z",
    ".gz",
    ".har",
    ".key",
    ".keys",
    ".mobileconfig",
    ".p12",
    ".pcap",
    ".pcapng",
    ".pem",
    ".pfx",
    ".tar",
    ".tgz",
    ".zip",
}
CAPTURE_MAGICS = (
    bytes.fromhex("a1b2c3d4"),
    bytes.fromhex("d4c3b2a1"),
    bytes.fromhex("a1b23c4d"),
    bytes.fromhex("4d3cb2a1"),
    bytes.fromhex("0a0d0d0a"),
    bytes.fromhex("504b0304"),
)
SENSITIVE_PATTERNS = (
    r"-----BEGIN [A-Z0-9 ]*(?:PRIVATE KEY|CERTIFICATE)-----",
    r"(?i)\b(?:authorization\s*:\s*)?bearer\s+[A-Za-z0-9._~+/-]{8,}",
    r"(?i)\b(?:api[_-]?key|secret|password|token)\s*[:=]\s*\S{4,}",
    r"(?i)\b(?:device[_-]?id|serial(?:_number)?|idfv|idfa)\s*[:=]\s*\S+",
    r"(?i)\bimei\s*[:=]\s*\d{14,16}\b",
    r"(?i)\budid\s*[:=]\s*[0-9a-f-]{24,64}\b",
    r"(?i)\b(?:device[_-]?)?mac\s*[:=]\s*(?:[0-9a-f]{2}:){5}[0-9a-f]{2}\b",
    r"(?i)[\"']?(?:latitude|longitude|lat|lon)[\"']?\s*[:=]\s*-?\d{1,3}\.\d{3,}",
    r"(?m)^(?:CLIENT_RANDOM|CLIENT_HANDSHAKE_TRAFFIC_SECRET|SERVER_HANDSHAKE_TRAFFIC_SECRET)\s",
    r"(?m)^(?:GET|POST|PUT|PATCH|DELETE)\s+\S+\s+HTTP/1\.[01]\r?$",
    r"(?m)^HTTP/1\.[01]\s+\d{3}\b",
    r"\bgh[pousr]_[A-Za-z0-9]{20,255}\b",
    r"\bgithub_pat_[A-Za-z0-9_]{20,255}\b",
    r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,255}\b",
    r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b",
    r"\beyJ[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{4,}\.[A-Za-z0-9_-]{4,}\b",
    r"(?i)(?<![0-9a-f])(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}(?![0-9a-f])",
    r"(?i)(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])",
    r"(?i)(?<![0-9a-f])[0-9a-f]{8}-[0-9a-f]{16}(?![0-9a-f])",
    r"(?i)(?<![0-9a-f])[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}(?![0-9a-f])",
    r"\[\s*-?\d{1,3}\.\d{3,}\s*,\s*-?\d{1,3}\.\d{3,}\s*\]",
    r"(?is)[\"']log[\"']\s*:\s*\{.*[\"']entries[\"']\s*:\s*\[",
)


class FixtureRejected(ValueError):
    """Stable-code rejection that never includes untrusted input."""

    def __init__(self, code: str) -> None:
        self.code = code
        super().__init__(code)


def _reject(code: str) -> None:
    raise FixtureRejected(code)


def _safe_ascii_name(name: str) -> bool:
    return bool(name) and len(name) <= 128 and all(0x21 <= ord(char) <= 0x7E for char in name)


def _read_regular(path: Path, maximum: int) -> bytes:
    """Read a stable regular file using no-follow directory-relative opens."""
    if not _safe_ascii_name(path.name):
        _reject("UnsafePath")
    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    file_flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    directory_fd = -1
    file_fd = -1
    try:
        directory_fd = os.open(str(path.parent), directory_flags)
        file_fd = os.open(path.name, file_flags, dir_fd=directory_fd)
        details = os.fstat(file_fd)
        if not stat.S_ISREG(details.st_mode):
            _reject("UnsafeFile")
        if not 1 <= details.st_size <= maximum:
            _reject("SizeLimit")
        chunks: list[bytes] = []
        remaining = details.st_size
        while remaining:
            chunk = os.read(file_fd, min(remaining, 64 * 1024))
            if not chunk:
                _reject("UnsafeFile")
            chunks.append(chunk)
            remaining -= len(chunk)
        if os.fstat(file_fd).st_size != details.st_size:
            _reject("UnsafeFile")
        return b"".join(chunks)
    except FixtureRejected:
        raise
    except (FileNotFoundError, NotADirectoryError, OSError):
        _reject("UnsafeFile")
    finally:
        if file_fd >= 0:
            os.close(file_fd)
        if directory_fd >= 0:
            os.close(directory_fd)
    raise AssertionError("unreachable")


def _no_duplicate_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _reject("InvalidJson")
        result[key] = value
    return result


def _parse_json(raw: bytes) -> Any:
    try:
        return json.loads(raw.decode("utf-8"), object_pairs_hook=_no_duplicate_object)
    except FixtureRejected:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError):
        _reject("InvalidJson")


def _check_depth(value: Any, current: int = 0) -> None:
    if current > MAX_JSON_DEPTH:
        _reject("SchemaLimit")
    if isinstance(value, dict):
        for child in value.values():
            _check_depth(child, current + 1)
    elif isinstance(value, list):
        for child in value:
            _check_depth(child, current + 1)


def _exact_object(value: Any, keys: set[str], code: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        _reject(code)
    return value


def _bounded_ascii(value: Any, maximum: int, code: str) -> str:
    if not isinstance(value, str) or not 1 <= len(value) <= maximum:
        _reject(code)
    if not value.strip() or any(ord(character) < 0x20 or ord(character) > 0x7E for character in value):
        _reject(code)
    return value


def _decoded_views(payload: bytes) -> list[str]:
    views = [payload.decode("latin-1", errors="ignore")]
    if b"\x00" in payload:
        for encoding in ("utf-16-le", "utf-16-be"):
            try:
                views.append(payload.decode(encoding))
            except UnicodeDecodeError:
                continue
    return views


def _valid_imei(candidate: str) -> bool:
    total = 0
    for index, character in enumerate(candidate):
        digit = int(character)
        if index % 2:
            digit *= 2
            if digit > 9:
                digit -= 9
        total += digit
    return total % 10 == 0


def inspect_payload(payload: bytes, filename: str) -> None:
    """Reject high-confidence secret, identifier, location, and capture signals."""
    if not isinstance(payload, bytes) or not 1 <= len(payload) <= MAX_PAYLOAD_BYTES:
        _reject("SizeLimit")
    if not _safe_ascii_name(filename):
        _reject("UnsafePath")
    if Path(filename).suffix.lower() in FORBIDDEN_SUFFIXES:
        _reject("ForbiddenFileType")
    if payload.startswith(CAPTURE_MAGICS):
        _reject("SensitiveContent")
    for text in _decoded_views(payload):
        for pattern in SENSITIVE_PATTERNS:
            if re.search(pattern, text):
                _reject("SensitiveContent")
        if any(_valid_imei(match.group(0)) for match in re.finditer(r"(?<!\d)\d{15}(?!\d)", text)):
            _reject("SensitiveContent")


def inspect_file(path: Path) -> bytes:
    payload = _read_regular(path, MAX_PAYLOAD_BYTES)
    inspect_payload(payload, path.name)
    return payload


def _validate_synthetic_manifest(manifest: Any) -> dict[str, Any]:
    top = _exact_object(
        manifest,
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
        },
        "InvalidManifest",
    )
    if top["schema_version"] != "wloc-fixture/v1":
        _reject("InvalidManifest")
    if top["classification"] != "synthetic":
        _reject("UnsupportedClassification")
    if top["hostname"] not in ALLOWED_HOSTNAMES or top["alpn"] != "h2":
        _reject("InvalidTransportMetadata")
    if top["ios_version"] != "not-applicable-synthetic":
        _reject("InvalidSyntheticMetadata")
    redactions = top["redactions"]
    if not isinstance(redactions, list) or any(not isinstance(item, str) for item in redactions):
        _reject("InvalidRedactions")
    if len(redactions) != len(set(redactions)) or not set(redactions).issubset(ALLOWED_REDACTIONS):
        _reject("InvalidRedactions")
    if redactions:
        _reject("InvalidSyntheticMetadata")

    fixture = _exact_object(
        top["fixture"], {"id", "path", "media_type", "byte_length", "sha256"}, "InvalidFixture"
    )
    fixture_id = _bounded_ascii(fixture["id"], 64, "InvalidFixture")
    if re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", fixture_id) is None:
        _reject("InvalidFixture")
    if fixture["path"] != "fixture.bin":
        _reject("UnsafePath")
    if fixture["media_type"] != "application/octet-stream":
        _reject("InvalidFixture")
    if (
        not isinstance(fixture["byte_length"], int)
        or isinstance(fixture["byte_length"], bool)
        or fixture["byte_length"] != SYNTHETIC_PAYLOAD_BYTES
    ):
        _reject("UnsupportedSyntheticPayload")
    if not isinstance(fixture["sha256"], str) or re.fullmatch(r"[0-9a-f]{64}", fixture["sha256"]) is None:
        _reject("InvalidFixture")

    provenance = _exact_object(
        top["provenance"],
        {"kind", "generator", "generator_version", "source_basis"},
        "InvalidProvenance",
    )
    expected_provenance = {
        "kind": "project-generated",
        "generator": SYNTHETIC_GENERATOR,
        "generator_version": SYNTHETIC_GENERATOR_VERSION,
        "source_basis": SYNTHETIC_SOURCE_BASIS,
    }
    if provenance != expected_provenance:
        _reject("InvalidProvenance")
    authorization = _exact_object(
        top["authorization"], {"status", "record_id"}, "InvalidAuthorization"
    )
    if authorization != {"status": "not-required-synthetic", "record_id": "not-applicable"}:
        _reject("InvalidAuthorization")
    return top


def validate_fixture(manifest_path: Path, schema_path: Path) -> dict[str, Any]:
    """Validate one fixed-format synthetic fixture with a pinned canonical schema."""
    schema_bytes = _read_regular(schema_path, MAX_SCHEMA_BYTES)
    if hashlib.sha256(schema_bytes).hexdigest() != TRUSTED_SCHEMA_SHA256:
        _reject("UntrustedSchema")
    schema = _parse_json(schema_bytes)
    if not isinstance(schema, dict) or schema.get("$schema") != "https://json-schema.org/draft/2020-12/schema":
        _reject("UntrustedSchema")

    if manifest_path.name != "manifest.json":
        _reject("UnsafePath")
    manifest_bytes = _read_regular(manifest_path, MAX_MANIFEST_BYTES)
    manifest = _parse_json(manifest_bytes)
    if isinstance(manifest, dict) and manifest.get("classification") == "authorized-sanitized-capture":
        _reject("AuthorizedCaptureGateClosed")
    inspect_payload(manifest_bytes, manifest_path.name)
    _check_depth(manifest)
    safe_manifest = _validate_synthetic_manifest(manifest)

    payload_path = manifest_path.parent / "fixture.bin"
    payload = inspect_file(payload_path)
    if len(payload) != safe_manifest["fixture"]["byte_length"]:
        _reject("HashMismatch")
    if hashlib.sha256(payload).hexdigest() != safe_manifest["fixture"]["sha256"]:
        _reject("HashMismatch")
    if len(payload) != SYNTHETIC_PAYLOAD_BYTES or not payload.startswith(SYNTHETIC_PREFIX):
        _reject("UnsupportedSyntheticPayload")
    return safe_manifest


def validate_inventory(fixtures_root: Path, schema_path: Path) -> None:
    """Ensure the fixture tree contains only governance files and registered synthetics."""
    try:
        root_details = fixtures_root.lstat()
    except OSError:
        _reject("InventoryViolation")
    if not stat.S_ISDIR(root_details.st_mode) or fixtures_root.is_symlink():
        _reject("InventoryViolation")
    try:
        entries = list(os.scandir(fixtures_root))
    except OSError:
        _reject("InventoryViolation")

    names = {entry.name for entry in entries}
    if not {"README.md", "schema"}.issubset(names):
        _reject("InventoryViolation")
    for entry in entries:
        if not _safe_ascii_name(entry.name) or entry.is_symlink():
            _reject("InventoryViolation")
        if entry.name == "README.md":
            if not entry.is_file(follow_symlinks=False):
                _reject("InventoryViolation")
            continue
        if entry.name == "schema":
            if not entry.is_dir(follow_symlinks=False):
                _reject("InventoryViolation")
            schema_entries = list(os.scandir(entry.path))
            if len(schema_entries) != 1 or schema_entries[0].name != "manifest.schema.json":
                _reject("InventoryViolation")
            continue
        if not entry.is_dir(follow_symlinks=False):
            _reject("InventoryViolation")
        children = list(os.scandir(entry.path))
        if {child.name for child in children} != {"manifest.json", "fixture.bin"}:
            _reject("InventoryViolation")
        if any(child.is_symlink() or not child.is_file(follow_symlinks=False) for child in children):
            _reject("InventoryViolation")
        validate_fixture(Path(entry.path) / "manifest.json", schema_path)


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate repository-safe synthetic fixtures")
    parser.add_argument("manifest", nargs="?", type=Path)
    parser.add_argument("--inventory", type=Path)
    parser.add_argument(
        "--schema",
        type=Path,
        default=Path(__file__).resolve().parents[2] / "fixtures" / "schema" / "manifest.schema.json",
    )
    arguments = parser.parse_args()
    try:
        if arguments.inventory is not None:
            if arguments.manifest is not None:
                parser.exit(2, "fixture rejected: InvalidCommand\n")
            validate_inventory(arguments.inventory, arguments.schema)
            print("fixture inventory accepted")
        elif arguments.manifest is not None:
            manifest = validate_fixture(arguments.manifest, arguments.schema)
            print(f"fixture accepted: {manifest['fixture']['id']} {manifest['fixture']['sha256']}")
        else:
            parser.exit(2, "fixture rejected: InvalidCommand\n")
    except FixtureRejected as error:
        parser.exit(1, f"fixture rejected: {error.code}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
