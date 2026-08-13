import pathlib
import subprocess
import unittest


class WlocTabLocalizationTests(unittest.TestCase):
    def test_tabs_stay_localized_after_faq_navigation(self) -> None:
        root = pathlib.Path(__file__).resolve().parents[1]
        completed = subprocess.run(
            ["node", "tests/js/tab_localization.test.js"],
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
