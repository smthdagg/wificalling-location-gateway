import importlib.util
import os
import pathlib
import subprocess
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("agent_state", ROOT / "scripts" / "agent_state.py")
agent_state = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(agent_state)


class StateEnvelopeTests(unittest.TestCase):
    def test_round_trip_is_strict(self):
        original = {"agent_id": "terra", "issue": 7, "state": "active"}
        encoded = agent_state.encode_state(agent_state.LEASE_MARKER, original)
        self.assertEqual(agent_state.parse_state(encoded, agent_state.LEASE_MARKER), original)

    def test_rejects_quoted_or_appended_marker_content(self):
        payload = (
            "quoted text\nagent-lease:v1\n"
            '{"agent_id":"attacker","issue":7,"state":"active"}\n'
        )
        with self.assertRaises(agent_state.StateError):
            agent_state.parse_state(payload, agent_state.LEASE_MARKER)

    def test_capability_tags_are_unique_and_non_secret_names(self):
        self.assertEqual(agent_state.parse_caps("go,test,tls-h2"), ["go", "test", "tls-h2"])
        for invalid in ("", "go,go", "go,has space", "go,$TOKEN"):
            with self.subTest(invalid=invalid):
                with self.assertRaises(agent_state.StateError):
                    agent_state.parse_caps(invalid)

    def test_capsule_identity_is_strict_and_unique(self):
        capsule = """# Agent handoff: Issue 9
- Source agent ID: terra
- Capabilities used: ci,security
- Branch: codex/issue-9-test-terra
- Credentials included: no
"""
        self.assertEqual(
            agent_state.parse_capsule(capsule),
            {
                "agent_id": "terra",
                "branch": "codex/issue-9-test-terra",
                "capabilities": ["ci", "security"],
                "credentials_included": "no",
                "issue": 9,
            },
        )
        with self.assertRaises(agent_state.StateError):
            agent_state.parse_capsule(capsule + "- Branch: codex/issue-9-forged\n")


class AtomicRefTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        base = pathlib.Path(self.temp.name)
        self.remote = base / "remote.git"
        self.work = base / "work"
        subprocess.run(["git", "init", "--bare", str(self.remote)], check=True, capture_output=True)
        subprocess.run(["git", "init", str(self.work)], check=True, capture_output=True)
        subprocess.run(["git", "-C", str(self.work), "config", "user.name", "Lease Test"], check=True)
        subprocess.run(["git", "-C", str(self.work), "config", "user.email", "lease@example.invalid"], check=True)
        subprocess.run(["git", "-C", str(self.work), "remote", "add", "origin", str(self.remote)], check=True)
        self.previous_cwd = pathlib.Path.cwd()
        os.chdir(self.work)

    def tearDown(self):
        os.chdir(self.previous_cwd)
        self.temp.cleanup()

    def test_force_with_lease_rejects_stale_generation(self):
        ref = "refs/heads/agent-leases/issue-7"
        first = agent_state.create_state_commit(
            ref, agent_state.LEASE_MARKER, {"issue": 7, "state": "active", "generation": 1}, None
        )
        second = agent_state.create_state_commit(
            ref, agent_state.LEASE_MARKER, {"issue": 7, "state": "active", "generation": 2}, first
        )
        self.assertNotEqual(first, second)
        with self.assertRaises(agent_state.StateError):
            agent_state.create_state_commit(
                ref, agent_state.LEASE_MARKER, {"issue": 7, "state": "active", "generation": 3}, first
            )
        remote_sha, state = agent_state.read_remote_state(ref, agent_state.LEASE_MARKER)
        self.assertEqual(remote_sha, second)
        self.assertEqual(state["generation"], 2)

    def test_atomic_handoff_cannot_overwrite_a_new_lease(self):
        lease_ref = "refs/heads/agent-leases/issue-9"
        handoff_ref = "refs/heads/agent-handoffs/issue-9"
        lease_a = agent_state.create_state_commit(
            lease_ref,
            agent_state.LEASE_MARKER,
            {"agent_id": "a", "issue": 9, "state": "active"},
            None,
        )
        lease_b = agent_state.create_state_commit(
            lease_ref,
            agent_state.LEASE_MARKER,
            {"agent_id": "b", "issue": 9, "state": "active"},
            lease_a,
        )
        stale_handoff = agent_state.build_state_commit(
            agent_state.HANDOFF_MARKER,
            {"agent_id": "a", "issue": 9},
            None,
        )
        stale_release = agent_state.build_state_commit(
            agent_state.LEASE_MARKER,
            {"agent_id": "a", "issue": 9, "state": "released"},
            lease_a,
        )
        with self.assertRaises(agent_state.StateError):
            agent_state.push_state_updates(
                [
                    (handoff_ref, stale_handoff, None),
                    (lease_ref, stale_release, lease_a),
                ]
            )
        self.assertIsNone(agent_state.remote_ref_sha(handoff_ref))
        self.assertEqual(agent_state.remote_ref_sha(lease_ref), lease_b)


if __name__ == "__main__":
    unittest.main()
