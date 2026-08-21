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

    def test_profile_page_exposes_basic_settings_and_low_frequency_status_contract(self):
        for relative in (
            "openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js",
            "openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js",
        ):
            source = (self.root / relative).read_text(encoding="utf-8")
            self.assertIn("Basic settings", source)
            self.assertIn("probe_interval", source)
            self.assertIn("reason_code", source)
            self.assertIn("Apply & restart", source)
            self.assertIn(", 15);", source)

    def test_profile_page_has_one_apply_boundary_and_bounded_input_guards(self):
        for relative in (
            "openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js",
            "openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js",
        ):
            source = (self.root / relative).read_text(encoding="utf-8")
            self.assertIn("validateProfiles", source)
            self.assertIn("probe interval must be between", source)
            self.assertIn("ui.changes.apply(true)", source)
            self.assertIn("removeProfile(section)", source)
            self.assertIn("normalizeDeviceAddress", source)
            self.assertIn("device address must be a private IPv4 address or unicast MAC", source)
            self.assertEqual(source.count("uci.save('wloc-service')"), 1)

    def test_monitor_reads_only_validated_profile_state_paths(self):
        for prefix in (
            self.root / "openwrt/files",
            self.root / "openwrt/luci-app-wificalling-location-gateway/files",
        ):
            monitor = (prefix / "www/luci-static/resources/view/wificalling-location-gateway/wloc-monitor.js").read_text(encoding="utf-8")
            acl = json.loads(
                (prefix / "usr/share/rpcd/acl.d/luci-app-wificalling-location-gateway.json").read_text(encoding="utf-8")
            )["luci-app-wificalling-location-gateway"]
            self.assertIn("selectedProfile", monitor)
            self.assertIn("wloc-service', 'device", monitor)
            self.assertIn("/var/run/wloc-service/profiles/", monitor)
            self.assertIn("[a-z0-9_]", monitor)
            files = acl["read"]["file"]
            self.assertIn("/var/run/wloc-service/profiles/*/status.json", files)
            self.assertIn("/var/run/wloc-service/profiles/*/events.jsonl", files)

    def test_restart_rpc_is_write_only_and_acl_sources_match(self):
        acl_sources = []
        for prefix in (
            self.root / "openwrt/files",
            self.root / "openwrt/luci-app-wificalling-location-gateway/files",
        ):
            acl = prefix / "usr/share/rpcd/acl.d/luci-app-wificalling-location-gateway.json"
            data = json.loads(acl.read_text(encoding="utf-8"))["luci-app-wificalling-location-gateway"]
            read_methods = data["read"]["ubus"]["luci.wloc"]
            write_methods = data["write"]["ubus"]["luci.wloc"]
            self.assertNotIn("restart_unified", read_methods)
            self.assertIn("restart_unified", write_methods)
            acl_sources.append(acl.read_text(encoding="utf-8"))
        self.assertEqual(acl_sources[0], acl_sources[1])

    def test_v2_ui_restart_actions_use_only_the_unified_lifecycle(self):
        for prefix in (
            self.root / "openwrt/files",
            self.root / "openwrt/luci-app-wificalling-location-gateway/files",
        ):
            rpc = (prefix / "usr/libexec/rpcd/luci.wloc").read_text(encoding="utf-8")
            wloc = (prefix / "www/luci-static/resources/view/wificalling-location-gateway/wloc.js").read_text(encoding="utf-8")
            health = (prefix / "www/luci-static/resources/view/wificalling-location-gateway/wloc-health.js").read_text(encoding="utf-8")
            restart_block = rpc[rpc.index("\trestart_service)"):rpc.index("\n\trestart_gateway)")]
            self.assertIn("/etc/init.d/wificalling-location-gateway restart", restart_block)
            self.assertNotIn("/etc/init.d/wloc-service restart", restart_block)
            self.assertNotIn("/etc/init.d/wificalling-gateway restart", restart_block)
            self.assertIn("method: 'restart_unified'", wloc)
            self.assertNotIn("method: 'restart_service'", wloc)
            self.assertIn("method: 'restart_unified'", health)
            self.assertNotIn("method: 'restart_gateway'", health)


if __name__ == "__main__":
    unittest.main()
