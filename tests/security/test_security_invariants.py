import json
from copy import deepcopy
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
SECURITY_DIR = ROOT / "docs" / "security"
CONTRACT_PATH = Path(__file__).with_name("security_invariants.json")


def assert_hard_security_semantics(testcase, threat_model, fail_open, contract):
    """Check critical policy against code-owned expectations, not contract-owned values."""
    testcase.assertEqual(
        contract["scope"],
        {
            "assigned_devices": 1,
            "hostnames": [
                "gs-loc.apple.com",
                "gs-loc-cn.apple.com",
                "gsp-ssl.ls.apple.com",
                "bluedot.is.autonavi.com",
                "bluedot.is.autonavi.com.gds.alibabadns.com",
                "gspe19-cn-ssl-ls-apple-com.v.aaplimg.com",
            ],
            "transport": "TCP 443",
            "dedicated_table": "wificalling_location",
            "protected_table": "wificalling_gateway",
            "forbidden_udp_ports": [500, 4500],
        },
    )

    combined = threat_model + fail_open
    hard_phrases = (
        "short-TTL nft set elements",
        "kernel-expiring lease",
        "supervisor renews the lease only while engine, scope, and IPv6 health pass",
        "kernel automatically expires the lease",
        "rule may remain present but cannot redirect without a matching live lease",
        "startup and reboot begin with no lease",
        "active stop still deletes the redirect and verifies absence",
        "never disable certificate verification",
        "never a default or fallback coordinate",
        "ipv6_mode_not_ready`: do not install redirect",
        "must never modify, reuse, flush, or depend on the `wificalling_gateway` table",
        "UDP 500/4500",
    )
    for phrase in hard_phrases:
        testcase.assertIn(phrase, combined)

    d03 = next(item for item in contract["critical_high_threats"] if item["id"] == "D-03")
    testcase.assertEqual(
        d03,
        {
            "id": "D-03",
            "control": "gate redirect matches on a kernel-expiring lease renewed only while engine, scope and IPv6 are healthy",
            "evidence": "AX6S kill, OOM and supervisor-loss tests measure kernel lease expiry and maximum blackhole time",
        },
    )


class SecurityInvariantDocumentationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
        threat_model_path = SECURITY_DIR / "threat-model.md"
        fail_open_path = SECURITY_DIR / "fail-open.md"
        cls.threat_model = threat_model_path.read_text(encoding="utf-8") if threat_model_path.exists() else ""
        cls.fail_open = fail_open_path.read_text(encoding="utf-8") if fail_open_path.exists() else ""

    def test_contract_is_versioned_and_scope_is_exact(self):
        self.assertEqual(self.contract["schema"], "wloc.security-invariants/v1")
        scope = self.contract["scope"]
        self.assertEqual(scope["assigned_devices"], 1)
        self.assertEqual(
            scope["hostnames"],
            [
                "gs-loc.apple.com",
                "gs-loc-cn.apple.com",
                "gsp-ssl.ls.apple.com",
                "bluedot.is.autonavi.com",
                "bluedot.is.autonavi.com.gds.alibabadns.com",
                "gspe19-cn-ssl-ls-apple-com.v.aaplimg.com",
            ],
        )
        self.assertEqual(scope["transport"], "TCP 443")
        self.assertEqual(scope["forbidden_udp_ports"], [500, 4500])
        self.assertEqual(scope["dedicated_table"], "wificalling_location")
        self.assertEqual(scope["protected_table"], "wificalling_gateway")

    def test_every_invariant_has_a_canonical_document_anchor(self):
        documents = {
            "threat-model.md": self.threat_model,
            "fail-open.md": self.fail_open,
        }
        for invariant in self.contract["invariants"]:
            marker = f'<!-- SECURITY_INVARIANT id="{invariant["id"]}" -->'
            with self.subTest(invariant=invariant["id"]):
                self.assertIn(marker, documents[invariant["document"]])

    def test_scope_excludes_gateway_and_wifi_calling_ipsec(self):
        required = (
            "one assigned test device",
            "gs-loc.apple.com",
            "gs-loc-cn.apple.com",
            "gsp-ssl.ls.apple.com",
            "bluedot.is.autonavi.com",
            "bluedot.is.autonavi.com.gds.alibabadns.com",
            "gspe19-cn-ssl-ls-apple-com.v.aaplimg.com",
            "TCP 443",
            "UDP 500/4500",
            "wificalling_location",
            "wificalling_gateway",
        )
        for text in required:
            with self.subTest(text=text):
                self.assertIn(text, self.threat_model)
        self.assertIn("must never modify", self.threat_model)

    def test_trust_boundaries_are_diagrammed(self):
        self.assertIn("```mermaid", self.threat_model)
        for boundary in ("Authorized test device", "TLS / ALPN / HTTP/2", "Apple WLOC upstream", "Geo providers", "watchdog"):
            with self.subTest(boundary=boundary):
                self.assertIn(boundary, self.threat_model)

    def test_each_critical_or_high_threat_maps_to_control_and_evidence(self):
        threats = self.contract["critical_high_threats"]
        self.assertGreaterEqual(len(threats), 15)
        self.assertEqual(len({item["id"] for item in threats}), len(threats))
        for threat in threats:
            with self.subTest(threat=threat["id"]):
                self.assertTrue(threat["control"].strip())
                self.assertTrue(threat["evidence"].strip())
                self.assertIn(f'| {threat["id"]} |', self.threat_model)

    def test_fail_open_dispositions_are_explicit(self):
        for cause, disposition in self.contract["failure_dispositions"].items():
            with self.subTest(cause=cause):
                self.assertIn(cause, self.fail_open)
                self.assertIn(disposition, self.fail_open)
        self.assertIn("never a default or fallback coordinate", self.fail_open)
        self.assertIn("verified original response", self.fail_open)

    def test_tls_h2_ca_and_resource_controls_are_explicit(self):
        required = (
            "TLS 1.2 and TLS 1.3",
            "ALPN",
            "HTTP/2",
            "upstream certificate",
            "leaf SAN",
            "0600",
        )
        for text in required + tuple(self.contract["resource_limits"]):
            with self.subTest(text=text):
                self.assertIn(text, self.threat_model)

    def test_ipv4_ipv6_decision_is_a_real_device_gate(self):
        for text in ("IPv4", "IPv6", "complete dual-stack", "scoped AAAA suppression", "must not globally disable IPv6"):
            with self.subTest(text=text):
                self.assertIn(text, self.threat_model)
        self.assertIn("do not install redirect", self.fail_open)

    def test_logs_and_support_bundles_use_an_allowlist(self):
        required = (
            "allowlist",
            "request or response bodies",
            "device MAC or IP",
            "precise coordinates",
            "CA or leaf private keys",
            "node credentials",
            "provider tokens",
            "1 MiB",
        )
        for text in required:
            with self.subTest(text=text):
                self.assertIn(text, self.threat_model)

    def test_emergency_and_compliance_limitations_are_unambiguous(self):
        required = (
            "does not guarantee emergency-call location",
            "does not certify carrier compliance",
            "does not prove Wi-Fi Calling activation",
            "authorized test device and LAN",
            "offline documentation and tests only",
        )
        combined = self.threat_model + self.fail_open
        for text in required:
            with self.subTest(text=text):
                self.assertIn(text, combined)

    def test_hard_security_semantics_do_not_trust_the_contract_as_oracle(self):
        assert_hard_security_semantics(
            self,
            self.threat_model,
            self.fail_open,
            self.contract,
        )

    def test_mutating_a_critical_phrase_or_scope_is_detected(self):
        mutations = (
            (
                self.threat_model,
                self.fail_open.replace(
                    "never disable certificate verification",
                    "disable certificate verification after a retry",
                ),
                self.contract,
            ),
            (
                self.threat_model,
                self.fail_open.replace(
                    "never a default or fallback coordinate",
                    "use a default coordinate when unavailable",
                ),
                self.contract,
            ),
            (
                self.threat_model,
                self.fail_open,
                {
                    **deepcopy(self.contract),
                    "scope": {**self.contract["scope"], "forbidden_udp_ports": [4500]},
                },
            ),
        )
        for threat_model, fail_open, contract in mutations:
            with self.subTest():
                with self.assertRaises(AssertionError):
                    assert_hard_security_semantics(
                        self,
                        threat_model,
                        fail_open,
                        contract,
                    )


if __name__ == "__main__":
    unittest.main()
