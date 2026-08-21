#!/usr/bin/env python3
"""Portable resource measurement fallback for macOS and minimal CI hosts."""

from __future__ import annotations

import argparse
import os
import resource
import subprocess
import time
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if not args.command or args.command[0] != "--":
        parser.error("command must follow --")
    command = args.command[1:]
    if not command:
        parser.error("command is required")
    if args.report.is_symlink():
        parser.error("report must not be a symlink")

    started = time.monotonic()
    completed = subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
    elapsed = time.monotonic() - started
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    rss = int(usage.ru_maxrss)
    if os.uname().sysname == "Darwin":
        rss //= 1024
    cpu = round(((usage.ru_utime + usage.ru_stime) / max(elapsed, 0.001)) * 100)
    report = "".join(
        (
            f"status={'pass' if completed.returncode == 0 else 'fail'}\n",
            f"elapsed_ms={round(elapsed * 1000)}\n",
            f"peak_rss_kib={rss}\n",
            f"cpu_percent={cpu}\n",
            f"command_status={completed.returncode}\n",
        )
    )
    args.report.write_text(report, encoding="ascii")
    args.report.chmod(0o600)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main())
