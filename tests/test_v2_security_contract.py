import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class V2SecurityContractTests(unittest.TestCase):
    def test_rpc_mutations_are_not_read_acl_methods(self):
        acl = json.loads(
            (ROOT / "openwrt/files/usr/share/rpcd/acl.d/luci-app-wificalling-location-gateway.json")
            .read_text(encoding="utf-8")
        )["luci-app-wificalling-location-gateway"]
        read_methods = set(acl["read"]["ubus"]["luci.wloc"])
        write_methods = set(acl["write"]["ubus"]["luci.wloc"])
        self.assertFalse({"ctl", "regen_profile", "regen_ca", "restart_service"} & read_methods)
        self.assertTrue({"ctl", "regen_profile", "regen_ca", "restart_service", "restart_unified"} <= write_methods)

    def test_rpc_lifecycle_never_kills_or_restarts_legacy_service(self):
        rpc = (ROOT / "openwrt/files/usr/libexec/rpcd/luci.wloc").read_text(encoding="utf-8")
        self.assertNotIn("killall", rpc)
        self.assertNotRegex(rpc, r"/etc/init\.d/wloc-service\s+restart")
        self.assertIn("/etc/init.d/wificalling-location-gateway restart", rpc)

    def test_integrated_package_declares_runtime_dependencies(self):
        makefile = (ROOT / "scripts/openwrt/build-release-packages.sh").read_text(encoding="utf-8")
        dependency_line = re.search(r"^\s*DEPENDS:=([^\n]+)$", makefile, re.MULTILINE)
        self.assertIsNotNone(dependency_line)
        dependencies = dependency_line.group(1)
        for dependency in (
            "+luci-base",
            "+rpcd-mod-rpcsys",
            "+nftables",
            "+firewall4",
            "+kmod-nft-tproxy",
            "+kmod-nft-socket",
            "+ip-full",
        ):
            self.assertIn(dependency, dependencies)

    def test_profile_ui_and_config_are_ipv4_only_until_mac_runtime_exists(self):
        ui = (ROOT / "openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js").read_text(encoding="utf-8")
        config = (ROOT / "openwrt/files/etc/config/wloc-service").read_text(encoding="utf-8")
        self.assertNotIn("unicast MAC", ui)
        self.assertNotIn("or MAC", ui)
        self.assertIn("private IPv4", ui)
        self.assertIn("IPv4", config)

    def test_provider_test_uses_the_configured_file(self):
        rpc = (ROOT / "openwrt/files/usr/libexec/rpcd/luci.wloc").read_text(encoding="utf-8")
        section = rpc.split("\tprovider_test)", 1)[1].split("\tupdate_status)", 1)[0]
        self.assertIn("check -c", section)


if __name__ == "__main__":
    unittest.main()
