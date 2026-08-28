"""Pure, non-deployable model for the Issue #6 traffic-isolation contract.

The returned dictionaries are review manifests, not nftables, dnsmasq, or procd
configuration.  They intentionally contain no command execution facility.
"""

from dataclasses import dataclass, replace
from enum import Enum
import ipaddress
from typing import Dict, Mapping, Tuple


APPROVED_HOSTS = (
    "gs-loc.apple.com",
    "gs-loc-cn.apple.com",
    "gsp-ssl.ls.apple.com",
    "bluedot.is.autonavi.com",
    "bluedot.is.autonavi.com.gds.alibabadns.com",
    "gspe19-cn-ssl-ls-apple-com.v.aaplimg.com",
)


def build_nft_plan(lease_ttl_seconds: int) -> Dict[str, object]:
    if not 5 <= lease_ttl_seconds <= 60:
        raise ValueError("offline lease TTL must be between 5 and 60 seconds")

    def family_scope() -> Dict[str, object]:
        return {
            "source_cardinality": 1,
            "destination_source": "exact-dns-allowlist",
            "transport": "tcp",
            "destination_port": 443,
            "lease": {
                "kind": "kernel-timeout-element",
                "ttl_seconds": lease_ttl_seconds,
            },
            "redirect_requires_live_lease": True,
        }

    return {
        "format": "offline-review-manifest/v1",
        "executable": False,
        "family": "inet",
        "table": "wificalling_location",
        "ownership": "project-only",
        "ipv6_mode": "full-dual-stack",
        "scope": {"ipv4": family_scope(), "ipv6": family_scope()},
        "teardown": [
            "remove-live-lease",
            "remove-owned-redirect",
            "verify-owned-redirect-absent",
            "remove-owned-table",
        ],
    }


def build_dns_plan(record_ttl_seconds: int) -> Dict[str, object]:
    if not 1 <= record_ttl_seconds <= 86400:
        raise ValueError("record TTL must be positive and bounded")
    return {
        "format": "offline-review-manifest/v1",
        "executable": False,
        "hostnames": list(APPROVED_HOSTS),
        "record_types": ["A", "AAAA"],
        "record_ttl_seconds": record_ttl_seconds,
        "update": "atomic-generation-replace",
        "expiry": "authoritative-record-ttl",
        "global_ipv6_disable": False,
        "aaaa_suppression": False,
        "reload_behavior": "rebuild-owned-destination-generations",
    }


def build_procd_plan() -> Dict[str, object]:
    return {
        "format": "offline-review-manifest/v1",
        "executable": False,
        "service": "wificalling-location",
        "supervision": "external-health-owner",
        "startup_lease": "absent",
        "reboot_lease": "absent",
        "renew_only_when": [
            "engine",
            "scope",
            "dns-a",
            "dns-aaaa",
            "ipv4",
            "ipv6",
            "watchdog",
        ],
        "fault_action": "stop-renewal-then-remove-owned-redirect",
        "respawn": "bounded-and-never-authoritative-for-health",
    }


@dataclass(frozen=True)
class Record:
    hostname: str
    record_type: str
    address: str
    expires_at: int


def reconcile_dns_generation(
    records: Tuple[Record, ...], now: int, generation: int
) -> Dict[str, object]:
    ipv4 = []
    ipv6 = []
    for record in records:
        if record.hostname not in APPROVED_HOSTS:
            raise ValueError("DNS record hostname is not exactly approved")
        if record.record_type not in ("A", "AAAA"):
            raise ValueError("only A and AAAA observations are accepted")
        address = ipaddress.ip_address(record.address)
        if (record.record_type == "A") != (address.version == 4):
            raise ValueError("DNS record type and address family disagree")
        if record.expires_at <= now:
            continue
        (ipv4 if address.version == 4 else ipv6).append(str(address))
    return {
        "generation": generation + 1,
        "replace": "atomic",
        "ipv4": sorted(set(ipv4)),
        "ipv6": sorted(set(ipv6)),
    }


@dataclass(frozen=True)
class Flow:
    address_family: str
    assigned_device: bool
    hostname: str
    transport: str
    destination_port: int
    live_lease: bool


def classify_flow(flow: Flow) -> str:
    exact_scope = (
        flow.address_family in ("ipv4", "ipv6")
        and flow.assigned_device
        and flow.hostname in APPROVED_HOSTS
        and flow.transport == "tcp"
        and flow.destination_port == 443
        and flow.live_lease
    )
    return "redirect" if exact_scope else "bypass"


class Event(Enum):
    VALIDATE_SCOPE = "validate-scope"
    VERIFY_DUAL_STACK = "verify-dual-stack"
    START_PASSTHROUGH = "start-pass-through"
    PROVE_ENGINE_HEALTH = "prove-engine-health"
    ARM_WATCHDOG = "arm-watchdog"
    INSTALL_REDIRECT = "install-redirect"
    RENEW_LEASE = "renew-lease"
    DISABLE = "disable"
    ENGINE_KILL = "engine-kill"
    ENGINE_OOM = "engine-oom"
    WATCHDOG_LOST = "watchdog-lost"
    LEASE_EXPIRED = "lease-expired"
    REBOOT = "reboot"


@dataclass(frozen=True)
class Lifecycle:
    state: str = "disabled"
    redirect_present: bool = False
    live_lease: bool = False
    engine_running: bool = False
    scope_valid: bool = False
    dual_stack_ready: bool = False
    engine_healthy: bool = False
    watchdog_armed: bool = False
    history: Tuple[str, ...] = ()

    @classmethod
    def startup(cls) -> "Lifecycle":
        return cls(history=("startup-no-lease",))

    def with_test_state(self, **changes: object) -> "Lifecycle":
        """Build synthetic failure states without exposing mutable production state."""
        return replace(self, **changes)

    def apply(self, event: Event) -> "Lifecycle":
        if event == Event.DISABLE:
            if self.state == "disabled" and not self.redirect_present and not self.live_lease:
                return self
            return replace(
                self,
                state="disabled",
                redirect_present=False,
                live_lease=False,
                engine_running=False,
                engine_healthy=False,
                watchdog_armed=False,
                history=self.history
                + ("remove-lease", "remove-redirect", "verify-absence", "stop-engine"),
            )
        if event == Event.REBOOT:
            return Lifecycle(history=self.history + ("reboot-no-lease",))
        if event in (Event.ENGINE_KILL, Event.ENGINE_OOM):
            return replace(
                self,
                state="degraded-pass-through",
                redirect_present=False,
                live_lease=False,
                engine_running=False,
                engine_healthy=False,
                history=self.history
                + ("stop-renewal", "remove-lease", "remove-redirect", "verify-absence"),
            )
        if event == Event.WATCHDOG_LOST:
            return replace(
                self,
                watchdog_armed=False,
                history=self.history + ("stop-renewal",),
            )
        if event == Event.LEASE_EXPIRED:
            return replace(
                self,
                state="degraded-pass-through",
                live_lease=False,
                history=self.history + ("kernel-lease-expired",),
            )

        transitions = {
            Event.VALIDATE_SCOPE: (
                not self.scope_valid,
                {"scope_valid": True, "state": "starting"},
            ),
            Event.VERIFY_DUAL_STACK: (
                self.scope_valid and not self.dual_stack_ready,
                {"dual_stack_ready": True},
            ),
            Event.START_PASSTHROUGH: (
                self.scope_valid and self.dual_stack_ready and not self.engine_running,
                {"engine_running": True, "state": "ready-pass-through"},
            ),
            Event.PROVE_ENGINE_HEALTH: (
                self.engine_running and not self.engine_healthy,
                {"engine_healthy": True},
            ),
            Event.ARM_WATCHDOG: (
                self.engine_healthy and not self.watchdog_armed and not self.live_lease,
                {"watchdog_armed": True},
            ),
            Event.INSTALL_REDIRECT: (
                self.scope_valid
                and self.dual_stack_ready
                and self.engine_healthy
                and self.watchdog_armed
                and not self.redirect_present
                and not self.live_lease,
                {"redirect_present": True},
            ),
            Event.RENEW_LEASE: (
                self.redirect_present
                and self.scope_valid
                and self.dual_stack_ready
                and self.engine_healthy
                and self.watchdog_armed,
                {"live_lease": True, "state": "intercepting"},
            ),
        }
        allowed, changes = transitions[event]
        if not allowed:
            raise ValueError("unsafe lifecycle transition: " + event.value)
        return replace(self, **changes, history=self.history + (event.value,))


def semantic_delta(
    before: Mapping[str, str], after: Mapping[str, str]
) -> Dict[str, object]:
    before_keys = set(before)
    after_keys = set(after)
    added = sorted(after_keys - before_keys)
    removed = sorted(before_keys - after_keys)
    changed = sorted(
        key for key in before_keys & after_keys if before[key] != after[key]
    )
    safe = (
        set(added) <= {"wificalling_location"}
        and set(removed) <= {"wificalling_location"}
        and set(changed) <= {"wificalling_location"}
    )
    return {"added": added, "removed": removed, "changed": changed, "safe": safe}
