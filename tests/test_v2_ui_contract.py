import json
import subprocess
import unittest
from pathlib import Path


class V2UiContractTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[1]

    def test_profile_pages_and_menu_are_present_in_both_package_sources(self):
        for prefix in (
            self.root / "openwrt/files",
            self.root / "openwrt/luci-app-wificalling-location-gateway/files",
        ):
            page = prefix / "www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js"
            health = prefix / "www/luci-static/resources/view/wificalling-location-gateway/wloc-health.js"
            menu = prefix / "usr/share/luci/menu.d/luci-app-wificalling-location-gateway.json"
            rpc = prefix / "usr/libexec/rpcd/luci.wloc"
            self.assertTrue(page.exists(), prefix)
            self.assertIn("restart_unified", rpc.read_text(encoding="utf-8"))
            self.assertIn("profiles", health.read_text(encoding="utf-8"))
            menu_data = json.loads(menu.read_text(encoding="utf-8"))
            self.assertIn(
                "admin/services/wificalling-location-gateway/wloc-devices",
                menu_data,
            )

    def test_profile_page_sources_parse_as_javascript(self):
        for relative in (
            "openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js",
            "openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js",
        ):
            result = subprocess.run(
                ["node", "--check", str(self.root / relative)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, result.returncode, result.stderr)


if __name__ == "__main__":
    unittest.main()
