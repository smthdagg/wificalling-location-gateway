# OpenWrt traffic-isolation test plan

This is a staged test plan, not a deployment script. Issue #6 runs only Stage
0. Stages 1–3 require later Issues and their explicit authorization.

| Stage | Environment | Evidence | Gate |
|---|---|---|---|
| 0 | unprivileged Python | declarative render, flow/state model, semantic diff | required for ADR review |
| 1 | isolated network namespace | actual nft/dns behavior; no host rules | required before QEMU |
| 2 | OpenWrt QEMU | procd, dnsmasq reload, reboot, kill/OOM | required before AX6S |
| 3 | authorized AX6S window | timeout measurements and semantic snapshots | required before device |

## Common matrix

- exact assigned IPv4 + each exact hostname + TCP 443 + live lease;
- exact assigned IPv6 + each exact hostname + TCP 443 + live lease;
- second device, source-binding drift, suffix/wildcard name, ordinary HTTPS;
- UDP 500/4500, router management, sing-box management and health traffic;
- A/AAAA addition, expiry, rotation, empty family, and dnsmasq reload;
- engine kill, engine OOM, supervisor kill/OOM, scheduler delay, crash loop;
- enable failure at every gate, repeated disable, rollback, and reboot;
- normalized before/enable/fault/disable/reboot semantic snapshots.

Stage 3 freezes the observed maximum from last successful lease renewal to
kernel expiry. With the 15-second TTL, the gate is at most 20 seconds. The test
must fail if a residual redirect remains active, any unrelated semantic hash
changes, or either Gateway/IPsec path records a WLOC redirect hit.

No stage may run on a real router or device merely because the offline tests
pass. Each stage needs its own Issue, rollback owner, and approval.
