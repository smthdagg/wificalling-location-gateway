#!/usr/bin/env python3
"""Pre-push high-confidence secret scanner; CI also runs pinned Gitleaks."""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys


PATTERNS = {
    "private key": re.compile(rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    "GitHub token": re.compile(rb"(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})"),
    "OpenAI-style key": re.compile(rb"\bsk-[A-Za-z0-9_-]{20,}\b"),
    "AWS access key": re.compile(rb"\bAKIA[0-9A-Z]{16}\b"),
    "JWT": re.compile(rb"\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b"),
    "assigned secret": re.compile(
        rb"(?i)(?:api[_ -]?key|access[_ -]?token|client[_ -]?secret|password)\s*[:=]\s*['\"][A-Za-z0-9_./+=-]{16,}['\"]"
    ),
}

SKIP = {"scripts/scan_secrets.py"}


def tracked_files() -> list[str]:
    result = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", "HEAD"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    return [line for line in result.stdout.splitlines() if line and line not in SKIP]


def main() -> int:
    findings: list[str] = []
    for name in tracked_files():
        path = pathlib.Path(name)
        # A worktree may intentionally remove a tracked legacy file before
        # the change is staged. There is no local byte content to scan; Git's
        # diff review remains the authority for the deletion itself.
        if not path.exists():
            continue
        try:
            data = path.read_bytes()
        except OSError as exc:
            findings.append(f"cannot scan {name}: {exc}")
            continue
        if len(data) > 2_000_000 or b"\0" in data:
            continue
        for label, pattern in PATTERNS.items():
            if pattern.search(data):
                findings.append(f"{name}: possible {label}")
    if findings:
        print("pre-push secret scan failed:", file=sys.stderr)
        for finding in findings:
            print(f"- {finding}", file=sys.stderr)
        return 1
    print("pre-push secret scan passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
