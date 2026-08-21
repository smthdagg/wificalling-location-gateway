import pathlib
import subprocess
import unittest


class WlocLuciModeSwitchTests(unittest.TestCase):
    def test_mode_switch_does_not_reenable_an_already_running_service(self) -> None:
        root = pathlib.Path(__file__).resolve().parents[1]
        sources = [
            root / "openwrt/files/usr/libexec/rpcd/luci.wloc",
            root
            / "openwrt/luci-app-wificalling-location-gateway/files/usr/libexec/rpcd/luci.wloc",
        ]
        for source in sources:
            text = source.read_text(encoding="utf-8")
            mode_block = text.split("\t\tmode-set)", 1)[1].split("\n\t\tgeo-search)", 1)[0]
            self.assertNotIn(
                "ctl_live enable",
                mode_block,
                msg=f"{source}: mode changes must not fail by enabling an active daemon again",
            )
            self.assertIn("ctl_live geo-set", mode_block)
            self.assertIn("ctl_live geo-clear", mode_block)

    def test_device_page_owns_manual_and_auto_location_fields(self) -> None:
        root = pathlib.Path(__file__).resolve().parents[1]
        for relative in (
            "openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js",
            "openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js",
        ):
            text = (root / relative).read_text(encoding="utf-8")
            self.assertIn("geoMode", text)
            self.assertIn("manual_lat", text)
            self.assertIn("manual_lon", text)
            self.assertIn("uci.set('wloc-service'", text)


if __name__ == "__main__":
    unittest.main()
