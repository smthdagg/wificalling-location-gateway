#!/usr/bin/env python3
"""Evaluate reproducible development-readiness gates for this repository."""

import argparse
import json
import shutil
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Callable, List, Optional, Sequence


COORDINATION_FILES = (
    "AGENTS.md",
    "DEVELOPMENT_TEST_PLAN.md",
    "SECURITY.md",
    "scripts/ci/verify.sh",
)
IMPLEMENTATION_FILES = (
    "go.mod",
    "fixtures/wloc/README.md",
    "docs/adr/0001-license-boundary.md",
    "docs/security/WLOC_THREAT_MODEL.md",
    "fixtures/wloc/manifest.json",
    "docs/protocol/WLOC_PROTOCOL_CONTRACT.md",
    "docs/adr/0002-ipv6-strategy.md",
    "docs/adr/0003-fail-open-slo.md",
)
RUST_CANDIDATE_FILES = (
    "Cargo.toml",
    "Cargo.lock",
    "src/lib.rs",
    "src/main.rs",
    "tests/rust_spike_contract.rs",
    "tests/rust_spike_policy.rs",
    "scripts/ci/verify-rust.sh",
    "docs/testing/RUST_ROUTE_AUDIT.md",
)
PHASE0_ACCEPTANCE_MARKERS = (
    (
        "phase0:license-adr-accepted",
        "docs/adr/0001-license-boundary.md",
        ("- Status: Accepted", "Status: **Accepted**", "状态：**Accepted**"),
    ),
    (
        "phase0:fixture-governance-accepted",
        "fixtures/wloc/README.md",
        ("- Status: Accepted", "Status: **Accepted**", "状态：**Accepted**"),
    ),
    (
        "phase0:threat-model-accepted",
        "docs/security/WLOC_THREAT_MODEL.md",
        ("- Status: Accepted", "Status: **Accepted**", "状态：**Accepted**"),
    ),
)
COORDINATION_TOOLS = ("git", "python3")
IMPLEMENTATION_TOOLS = ("go", "make", "openssl", "shellcheck")
RUST_CANDIDATE_TOOLS = ("cargo", "rustc", "make", "openssl")
PROFILES = ("coordination", "implementation", "rust-candidate")


@dataclass(frozen=True)
class Check:
    name: str
    ok: bool
    detail: str


@dataclass(frozen=True)
class Report:
    profile: str
    root: str
    checks: Sequence[Check]

    @property
    def blockers(self) -> List[Check]:
        return [check for check in self.checks if not check.ok]

    @property
    def ready(self) -> bool:
        return not self.blockers

    def to_dict(self):
        return {
            "profile": self.profile,
            "root": self.root,
            "ready": self.ready,
            "checks": [asdict(check) for check in self.checks],
            "blockers": [asdict(check) for check in self.blockers],
        }


def _tool_checks(
    tools: Sequence[str], lookup: Callable[[str], Optional[str]]
):
    checks = []
    for tool in tools:
        location = lookup(tool)
        checks.append(
            Check(
                name=f"tool:{tool}",
                ok=bool(location),
                detail=location or "not found in PATH",
            )
        )
    return tuple(checks)


def _file_checks(root: Path, files: Sequence[str]):
    checks = []
    for relative_path in files:
        path = root / relative_path
        ok = path.is_file() and path.stat().st_size > 0
        checks.append(
            Check(
                name=f"file:{relative_path}",
                ok=ok,
                detail=str(path) if ok else "missing or empty",
            )
        )
    return tuple(checks)


def _phase0_acceptance_checks(root: Path):
    checks = []
    for name, relative_path, markers in PHASE0_ACCEPTANCE_MARKERS:
        path = root / relative_path
        content = path.read_text(encoding="utf-8") if path.is_file() else ""
        ok = any(marker in content for marker in markers)
        checks.append(
            Check(
                name=name,
                ok=ok,
                detail=(
                    f"{relative_path} accepted"
                    if ok
                    else f"{relative_path} is missing accepted status"
                ),
            )
        )
    return tuple(checks)


def evaluate(
    root: Path,
    profile: str,
    lookup: Callable[[str], Optional[str]] = shutil.which,
) -> Report:
    if profile not in PROFILES:
        raise ValueError(f"unknown profile: {profile}")

    resolved_root = Path(root).resolve()
    tools = COORDINATION_TOOLS
    files = COORDINATION_FILES
    if profile == "implementation":
        tools += IMPLEMENTATION_TOOLS
        files += IMPLEMENTATION_FILES
    if profile == "rust-candidate":
        tools += RUST_CANDIDATE_TOOLS
        files += RUST_CANDIDATE_FILES

    checks = _tool_checks(tools, lookup) + _file_checks(resolved_root, files)
    if profile == "implementation":
        checks += _phase0_acceptance_checks(resolved_root)
    return Report(profile=profile, root=str(resolved_root), checks=checks)


def _render_text(report: Report) -> str:
    lines = [
        f"development readiness: {report.profile}",
        f"result: {'READY' if report.ready else 'BLOCKED'}",
    ]
    for check in report.checks:
        lines.append(f"[{'PASS' if check.ok else 'FAIL'}] {check.name}: {check.detail}")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=PROFILES, default="coordination")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--json", action="store_true", help="emit a JSON report")
    args = parser.parse_args()

    report = evaluate(args.root, args.profile)
    if args.json:
        print(json.dumps(report.to_dict(), ensure_ascii=False, indent=2))
    else:
        print(_render_text(report))
    return 0 if report.ready else 2


if __name__ == "__main__":
    raise SystemExit(main())
