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

    def test_mode_switch_serializes_save_apply_and_runtime_control(self) -> None:
        root = pathlib.Path(__file__).resolve().parents[1]
        completed = subprocess.run(
            ["node", "tests/js/wloc_mode_switch.test.js"],
            cwd=root,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(
            completed.returncode,
            0,
            msg=f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}",
        )


if __name__ == "__main__":
    unittest.main()
