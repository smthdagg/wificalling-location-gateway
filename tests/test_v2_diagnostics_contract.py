import json
import subprocess
import unittest
from pathlib import Path


class V2DiagnosticsContractTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[1]

    def test_support_bundle_is_bounded_and_privacy_safe(self):
        script = self.root / "openwrt/files/usr/sbin/wloc-support-bundle.sh"
        source = script.read_text(encoding="utf-8")
        self.assertIn("MAX_BYTES", source)
        self.assertIn("no-credentials-no-device-identifiers-no-precise-location", source)
        self.assertIn("redacted diagnostic event", source)
        self.assertIn("tar -czf", source)
        self.assertIn("wloc-support-bundle.lock", source)

    def test_support_bundle_is_installed_by_all_openwrt_package_paths(self):
        makefile = (self.root / "openwrt/Makefile").read_text(encoding="utf-8")
        standalone = (self.root / "scripts/build-luci-ipk.sh").read_text(encoding="utf-8")
        release = (self.root / "scripts/openwrt/build-release-packages.sh").read_text(encoding="utf-8")
        self.assertIn("files/usr/sbin/wloc-support-bundle.sh", makefile)
        self.assertIn("wloc-support-bundle.sh", standalone)
        self.assertIn("wloc-support-bundle.sh", release)

    def test_diagnostics_rpc_acl_and_ui_are_present_in_both_package_sources(self):
        for prefix in (
            self.root / "openwrt/files",
            self.root / "openwrt/luci-app-wificalling-location-gateway/files",
        ):
            rpc = (prefix / "usr/libexec/rpcd/luci.wloc").read_text(encoding="utf-8")
            self.assertIn("support_bundle", rpc)
            acl = json.loads(
                (prefix / "usr/share/rpcd/acl.d/luci-app-wificalling-location-gateway.json").read_text(
                    encoding="utf-8"
                )
            )["luci-app-wificalling-location-gateway"]
            read_methods = acl["read"]["ubus"]["luci.wloc"]
            write_methods = acl["write"]["ubus"]["luci.wloc"]
            self.assertNotIn("support_bundle", read_methods)
            self.assertIn("support_bundle", write_methods)
            health = (prefix / "www/luci-static/resources/view/wificalling-location-gateway/wloc-health.js").read_text(encoding="utf-8")
            self.assertIn("Generate support bundle", health)
            monitor = (prefix / "www/luci-static/resources/view/wificalling-location-gateway/wfc-monitor.js").read_text(encoding="utf-8")
            self.assertIn("eventFields", monitor)

    def test_support_bundle_shell_and_log_regression_scripts_pass(self):
        for name in ("test-support-bundle.sh", "test-structured-logs.sh"):
            result = subprocess.run(
                ["sh", str(self.root / "tests/scripts" / name)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
