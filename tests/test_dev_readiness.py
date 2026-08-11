import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

import dev_readiness  # noqa: E402


class DevelopmentReadinessTests(unittest.TestCase):
    def make_repo(self, files):
        temp_dir = tempfile.TemporaryDirectory()
        root = Path(temp_dir.name)
        for relative_path in files:
            path = root / relative_path
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("test\n", encoding="utf-8")
        self.addCleanup(temp_dir.cleanup)
        return root

    def test_coordination_profile_is_ready_with_baseline_files_and_tools(self):
        root = self.make_repo(dev_readiness.COORDINATION_FILES)

        report = dev_readiness.evaluate(
            root, "coordination", lookup=lambda _tool: "/usr/bin/tool"
        )

        self.assertTrue(report.ready)
        self.assertEqual([], report.blockers)

    def test_implementation_profile_reports_every_missing_gate(self):
        root = self.make_repo(dev_readiness.COORDINATION_FILES)

        report = dev_readiness.evaluate(
            root,
            "implementation",
            lookup=lambda tool: (
                None if tool in {"go", "shellcheck"} else "/usr/bin/tool"
            ),
        )

        blocker_names = {check.name for check in report.blockers}
        self.assertIn("tool:go", blocker_names)
        self.assertIn("tool:shellcheck", blocker_names)
        self.assertIn("file:go.mod", blocker_names)
        self.assertIn("file:fixtures/wloc/README.md", blocker_names)
        self.assertIn("file:docs/adr/0001-license-boundary.md", blocker_names)
        self.assertIn("file:docs/security/WLOC_THREAT_MODEL.md", blocker_names)
        self.assertIn("file:fixtures/wloc/manifest.json", blocker_names)
        self.assertIn("file:docs/protocol/WLOC_PROTOCOL_CONTRACT.md", blocker_names)
        self.assertIn("file:docs/adr/0002-ipv6-strategy.md", blocker_names)
        self.assertIn("file:docs/adr/0003-fail-open-slo.md", blocker_names)

    def test_implementation_profile_blocks_on_unaccepted_phase0_documents(self):
        root = self.make_repo(dev_readiness.COORDINATION_FILES + dev_readiness.IMPLEMENTATION_FILES)
        (root / "docs/adr/0001-license-boundary.md").write_text(
            "- Status: Proposed — requires protocol and security review\n",
            encoding="utf-8",
        )
        (root / "fixtures/wloc/README.md").write_text(
            "状态：**Phase 0 评审草案**\n",
            encoding="utf-8",
        )
        (root / "docs/security/WLOC_THREAT_MODEL.md").write_text(
            "状态：**Phase 0 评审草案；未批准真机接入**\n",
            encoding="utf-8",
        )

        report = dev_readiness.evaluate(
            root, "implementation", lookup=lambda _tool: "/usr/bin/tool"
        )

        blocker_names = {check.name for check in report.blockers}
        self.assertIn("phase0:license-adr-accepted", blocker_names)
        self.assertIn("phase0:fixture-governance-accepted", blocker_names)
        self.assertIn("phase0:threat-model-accepted", blocker_names)

    def test_cli_json_is_machine_readable_and_nonzero_when_blocked(self):
        result = subprocess.run(
            [
                sys.executable,
                str(REPO_ROOT / "scripts" / "dev_readiness.py"),
                "--profile",
                "implementation",
                "--root",
                str(REPO_ROOT),
                "--json",
            ],
            check=False,
            capture_output=True,
            text=True,
        )

        payload = json.loads(result.stdout)
        self.assertEqual(2, result.returncode)
        self.assertFalse(payload["ready"])
        self.assertTrue(payload["blockers"])

    def test_rust_candidate_profile_requires_rust_spike_files_and_tools(self):
        root = self.make_repo(dev_readiness.COORDINATION_FILES)

        report = dev_readiness.evaluate(
            root,
            "rust-candidate",
            lookup=lambda tool: None if tool == "cargo" else "/usr/bin/tool",
        )

        blocker_names = {check.name for check in report.blockers}
        self.assertIn("tool:cargo", blocker_names)
        self.assertIn("file:Cargo.toml", blocker_names)
        self.assertIn("file:Cargo.lock", blocker_names)
        self.assertIn("file:docs/testing/RUST_ROUTE_AUDIT.md", blocker_names)

    def test_rust_candidate_profile_is_ready_with_spike_files_and_tools(self):
        root = self.make_repo(
            dev_readiness.COORDINATION_FILES + dev_readiness.RUST_CANDIDATE_FILES
        )

        report = dev_readiness.evaluate(
            root, "rust-candidate", lookup=lambda _tool: "/usr/bin/tool"
        )

        self.assertTrue(report.ready)

    def test_unknown_profile_is_rejected(self):
        with self.assertRaises(ValueError):
            dev_readiness.evaluate(REPO_ROOT, "unknown")


if __name__ == "__main__":
    unittest.main()
