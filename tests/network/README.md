# tests/network

The original Issue #6 traffic-isolation model and its tests were removed in
the v1.3.0-r16 audit: they asserted a self-referential model that encoded the
withdrawn v1.3.0-r14 IPv4/IPv6 dual-stack design (full IPv6 interception
scope, AAAA DNS rotation) that this project deliberately did NOT adopt. The
shipped contract is IPv4-first TPROXY with an IPv6 deny-set only, and it is
pinned by `tests/scripts/test-wloc-runtime-contract.sh` against the real
installed scripts rather than an in-repo model.
