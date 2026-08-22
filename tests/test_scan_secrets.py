import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("scan_secrets", ROOT / "scripts" / "scan_secrets.py")
scan_secrets = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(scan_secrets)


class SecretScannerTests(unittest.TestCase):
    def test_input_derived_password_is_not_treated_as_embedded_secret(self):
        source = b"out.password = decodeURIComponent(url.username || '');"
        self.assertIsNone(scan_secrets.PATTERNS["assigned secret"].search(source))

    def test_literal_password_is_still_detected(self):
        source = b"password = 'not-a-real-but-long-secret-value';"
        self.assertIsNotNone(scan_secrets.PATTERNS["assigned secret"].search(source))
