import json
import unittest

from tests.network.traffic_isolation_model import (
    APPROVED_HOSTS,
    Event,
    Flow,
    Lifecycle,
    Record,
    build_dns_plan,
    build_nft_plan,
    build_procd_plan,
    classify_flow,
    reconcile_dns_generation,
    semantic_delta,
)


class RenderedPlanTests(unittest.TestCase):
    def setUp(self):
        self.nft = build_nft_plan(lease_ttl_seconds=15)
        self.dns = build_dns_plan(record_ttl_seconds=60)
        self.procd = build_procd_plan()
        self.rendered = json.dumps(
            {"nft": self.nft, "dns": self.dns, "procd": self.procd},
            sort_keys=True,
        )

    def test_plan_owns_one_dedicated_table_and_never_gateway_or_global_state(self):
        self.assertEqual(self.nft["table"], "wificalling_location")
        self.assertEqual(self.nft["family"], "inet")
        self.assertEqual(self.nft["ownership"], "project-only")
        forbidden = (
            "wificalling_gateway",
            "flush ruleset",
            '"table": "filter"',
            '"table": "nat"',
        )
        for token in forbidden:
            self.assertNotIn(token, self.rendered)

    def test_nft_plan_has_symmetric_ipv4_ipv6_scope_and_kernel_leases(self):
        self.assertEqual(self.nft["ipv6_mode"], "full-dual-stack")
        for family in ("ipv4", "ipv6"):
            scope = self.nft["scope"][family]
            self.assertEqual(scope["source_cardinality"], 1)
            self.assertEqual(scope["destination_source"], "exact-dns-allowlist")
            self.assertEqual(scope["transport"], "tcp")
            self.assertEqual(scope["destination_port"], 443)
            self.assertEqual(scope["lease"]["kind"], "kernel-timeout-element")
            self.assertEqual(scope["lease"]["ttl_seconds"], 15)
            self.assertTrue(scope["redirect_requires_live_lease"])

    def test_dns_plan_tracks_only_exact_a_and_aaaa_records_with_ttl_rotation(self):
        self.assertEqual(tuple(self.dns["hostnames"]), APPROVED_HOSTS)
        self.assertEqual(self.dns["record_types"], ["A", "AAAA"])
        self.assertEqual(self.dns["update"], "atomic-generation-replace")
        self.assertEqual(self.dns["expiry"], "authoritative-record-ttl")
        self.assertEqual(self.dns["record_ttl_seconds"], 60)
        self.assertFalse(self.dns["global_ipv6_disable"])
        self.assertFalse(self.dns["aaaa_suppression"])

    def test_procd_plan_cannot_create_or_renew_lease_without_all_health_gates(self):
        self.assertEqual(self.procd["startup_lease"], "absent")
        self.assertEqual(self.procd["reboot_lease"], "absent")
        self.assertEqual(
            self.procd["renew_only_when"],
            ["engine", "scope", "dns-a", "dns-aaaa", "ipv4", "ipv6", "watchdog"],
        )
        self.assertEqual(self.procd["fault_action"], "stop-renewal-then-remove-owned-redirect")

    def test_rendered_plans_contain_no_ipsec_or_broad_redirect(self):
        for token in ("udp", "500", "4500", "0.0.0.0/0", "::/0", "all-https"):
            self.assertNotIn(token, self.rendered.lower())

    def test_lease_ttl_is_short_and_bounded(self):
        for invalid_ttl in (0, 4, 61, 3600):
            with self.assertRaises(ValueError):
                build_nft_plan(lease_ttl_seconds=invalid_ttl)


class FlowIsolationTests(unittest.TestCase):
    def test_only_assigned_ipv4_wloc_tcp443_with_live_lease_redirects(self):
        flow = Flow("ipv4", True, "gs-loc.apple.com", "tcp", 443, True)
        self.assertEqual(classify_flow(flow), "redirect")

    def test_only_assigned_ipv6_wloc_tcp443_with_live_lease_redirects(self):
        flow = Flow("ipv6", True, "gs-loc-cn.apple.com", "tcp", 443, True)
        self.assertEqual(classify_flow(flow), "redirect")

    def test_other_devices_and_ordinary_https_bypass(self):
        cases = (
            Flow("ipv4", False, "gs-loc.apple.com", "tcp", 443, True),
            Flow("ipv6", False, "gs-loc-cn.apple.com", "tcp", 443, True),
            Flow("ipv4", True, "example.com", "tcp", 443, True),
            Flow("ipv6", True, "apple.com", "tcp", 443, True),
            Flow("ipv4", True, "evil.gs-loc.apple.com", "tcp", 443, True),
        )
        self.assertTrue(all(classify_flow(case) == "bypass" for case in cases))


class DnsRotationTests(unittest.TestCase):
    def test_a_and_aaaa_rotation_replaces_generation_and_drops_expired_records(self):
        records = (
            Record("gs-loc.apple.com", "A", "192.0.2.10", 1060),
            Record("gs-loc.apple.com", "AAAA", "2001:db8::10", 1060),
            Record("gs-loc-cn.apple.com", "A", "192.0.2.20", 999),
            Record("gs-loc-cn.apple.com", "AAAA", "2001:db8::20", 1060),
        )
        generation = reconcile_dns_generation(records, now=1000, generation=8)
        self.assertEqual(generation["generation"], 9)
        self.assertEqual(generation["ipv4"], ["192.0.2.10"])
        self.assertEqual(generation["ipv6"], ["2001:db8::10", "2001:db8::20"])
        self.assertEqual(generation["replace"], "atomic")

    def test_dns_rotation_rejects_nonexact_host_or_record_type(self):
        invalid = (
            Record("evil.gs-loc.apple.com", "A", "192.0.2.10", 1060),
            Record("gs-loc.apple.com", "CNAME", "alias.example", 1060),
        )
        for record in invalid:
            with self.assertRaises(ValueError):
                reconcile_dns_generation((record,), now=1000, generation=1)

    def test_dns_rotation_rejects_record_type_address_family_mismatch(self):
        with self.assertRaises(ValueError):
            reconcile_dns_generation(
                (Record("gs-loc.apple.com", "A", "2001:db8::10", 1060),),
                now=1000,
                generation=1,
            )

    def test_ipsec_and_missing_lease_always_bypass(self):
        cases = (
            Flow("ipv4", True, "gs-loc.apple.com", "udp", 500, True),
            Flow("ipv6", True, "gs-loc.apple.com", "udp", 4500, True),
            Flow("ipv4", True, "gs-loc.apple.com", "tcp", 443, False),
            Flow("ipv6", True, "gs-loc-cn.apple.com", "tcp", 443, False),
        )
        self.assertTrue(all(classify_flow(case) == "bypass" for case in cases))


class LifecycleTests(unittest.TestCase):
    def test_redirect_cannot_install_before_scope_dual_stack_engine_and_watchdog(self):
        with self.assertRaises(ValueError):
            Lifecycle.startup().apply(Event.INSTALL_REDIRECT)

    def test_enable_installs_redirect_last_and_renews_lease_after_it(self):
        lifecycle = Lifecycle.startup()
        for event in (
            Event.VALIDATE_SCOPE,
            Event.VERIFY_DUAL_STACK,
            Event.START_PASSTHROUGH,
            Event.PROVE_ENGINE_HEALTH,
            Event.ARM_WATCHDOG,
            Event.INSTALL_REDIRECT,
            Event.RENEW_LEASE,
        ):
            lifecycle = lifecycle.apply(event)
        self.assertEqual(lifecycle.state, "intercepting")
        self.assertEqual(lifecycle.history[-2:], ("install-redirect", "renew-lease"))
        self.assertTrue(lifecycle.redirect_present)
        self.assertTrue(lifecycle.live_lease)

    def test_startup_and_reboot_never_restore_a_lease(self):
        self.assertFalse(Lifecycle.startup().live_lease)
        running = Lifecycle.startup().with_test_state(redirect_present=True, live_lease=True)
        rebooted = running.apply(Event.REBOOT)
        self.assertFalse(rebooted.redirect_present)
        self.assertFalse(rebooted.live_lease)
        self.assertEqual(rebooted.state, "disabled")

    def test_disable_removes_lease_and_redirect_before_process_stop_and_is_idempotent(self):
        lifecycle = Lifecycle.startup().with_test_state(
            state="intercepting", redirect_present=True, live_lease=True, engine_running=True
        )
        disabled = lifecycle.apply(Event.DISABLE)
        self.assertEqual(
            disabled.history[-4:],
            ("remove-lease", "remove-redirect", "verify-absence", "stop-engine"),
        )
        self.assertEqual(disabled.apply(Event.DISABLE), disabled)

    def test_engine_kill_and_oom_remove_owned_redirect_immediately(self):
        for event in (Event.ENGINE_KILL, Event.ENGINE_OOM):
            lifecycle = Lifecycle.startup().with_test_state(
                state="intercepting", redirect_present=True, live_lease=True, engine_running=True
            )
            failed = lifecycle.apply(event)
            self.assertFalse(failed.redirect_present)
            self.assertFalse(failed.live_lease)
            self.assertEqual(failed.state, "degraded-pass-through")

    def test_watchdog_loss_stops_renewal_and_kernel_expiry_makes_residual_rule_inert(self):
        lifecycle = Lifecycle.startup().with_test_state(
            state="intercepting", redirect_present=True, live_lease=True, engine_running=True
        )
        lost = lifecycle.apply(Event.WATCHDOG_LOST)
        self.assertTrue(lost.redirect_present)
        self.assertTrue(lost.live_lease)
        expired = lost.apply(Event.LEASE_EXPIRED)
        self.assertTrue(expired.redirect_present)
        self.assertFalse(expired.live_lease)
        self.assertEqual(expired.state, "degraded-pass-through")


class SemanticProofTests(unittest.TestCase):
    def test_before_after_delta_is_limited_to_owned_objects(self):
        before = {
            "wificalling_gateway": "gateway-semantic-hash",
            "unrelated_filter": "filter-semantic-hash",
        }
        after = {
            **before,
            "wificalling_location": "location-semantic-hash",
        }
        delta = semantic_delta(before, after)
        self.assertEqual(delta["added"], ["wificalling_location"])
        self.assertEqual(delta["removed"], [])
        self.assertEqual(delta["changed"], [])
        self.assertTrue(delta["safe"])

    def test_semantic_proof_rejects_gateway_or_unrelated_changes(self):
        before = {"wificalling_gateway": "a", "unrelated_filter": "x"}
        after = {"wificalling_gateway": "b", "unrelated_filter": "x"}
        self.assertFalse(semantic_delta(before, after)["safe"])


if __name__ == "__main__":
    unittest.main()
