#!/usr/bin/env python3
"""Generate deterministic, generic fixture-governance bytes without networking."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
from pathlib import Path


GENERATOR_VERSION = "1"
HOSTNAMES = (
    "gs-loc.apple.com",
    "gs-loc-cn.apple.com",
    "gs-loc-corpa.apple.com",
    "gs-loc.apple.com.cn",
    "bluedot.is.autonavi.com",
    "bluedot.is.autonavi.com.gds.alibabadns.com",
)
SYNTHETIC_PREFIX = b"WLG-SYNTHETIC-GOVERNANCE-V1\x00"


def bounded_ascii(value: str, name: str, maximum: int) -> str:
    if not value or len(value) > maximum or any(ord(char) < 0x20 or ord(char) > 0x7E for char in value):
        raise ValueError(f"{name} must be 1..{maximum} printable ASCII characters")
    return value


def build_payload(seed: str) -> bytes:
    """Return generic bytes that make no private-protocol compatibility claim."""
    seed_digest = hashlib.sha256(seed.encode("ascii")).digest()
    return SYNTHETIC_PREFIX + seed_digest


def _exclusive_write(directory_fd: int, name: str, content: bytes) -> None:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    descriptor = os.open(name, flags, 0o600, dir_fd=directory_fd)
    try:
        position = 0
        while position < len(content):
            written = os.write(descriptor, content[position:])
            if written <= 0:
                raise ValueError("exclusive output write failed")
            position += written
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def generate(output_dir: Path, fixture_id: str, seed: str, hostname: str) -> dict:
    bounded_ascii(seed, "seed", 128)
    if re.fullmatch(r"[a-z0-9][a-z0-9._-]{0,63}", fixture_id) is None:
        raise ValueError("fixture id has unsafe characters")
    if hostname not in HOSTNAMES:
        raise ValueError("hostname is outside the exact allowlist")
    try:
        details = output_dir.lstat()
    except FileNotFoundError:
        output_dir.mkdir(mode=0o700)
    else:
        if output_dir.is_symlink() or not stat.S_ISDIR(details.st_mode):
            raise ValueError("output directory must be a real directory")

    directory_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        directory_fd = os.open(str(output_dir), directory_flags)
    except OSError as error:
        raise ValueError("output directory cannot be opened safely") from error
    try:
        if os.listdir(directory_fd):
            raise ValueError("output directory must be empty")

        payload = build_payload(seed)
        payload_name = "fixture.bin"
        manifest = {
            "schema_version": "wloc-fixture/v1",
            "fixture": {
                "id": fixture_id,
                "path": payload_name,
                "media_type": "application/octet-stream",
                "byte_length": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
            },
            "provenance": {
                "kind": "project-generated",
                "generator": "scripts/fixtures/generate_synthetic.py",
                "generator_version": GENERATOR_VERSION,
                "source_basis": "project-authored generic bytes; no capture or private protocol source",
            },
            "authorization": {
                "status": "not-required-synthetic",
                "record_id": "not-applicable",
            },
            "ios_version": "not-applicable-synthetic",
            "hostname": hostname,
            "alpn": "h2",
            "classification": "synthetic",
            "redactions": [],
        }
        manifest_bytes = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode("utf-8")
        _exclusive_write(directory_fd, payload_name, payload)
        _exclusive_write(directory_fd, "manifest.json", manifest_bytes)
    finally:
        os.close(directory_fd)
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--fixture-id", required=True)
    parser.add_argument("--seed", required=True)
    parser.add_argument("--hostname", required=True, choices=HOSTNAMES)
    arguments = parser.parse_args()
    try:
        generate(arguments.output_dir, arguments.fixture_id, arguments.seed, arguments.hostname)
    except ValueError as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
