import pathlib
import subprocess
import unittest


class WlocLuciModeSwitchTests(unittest.TestCase):
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
