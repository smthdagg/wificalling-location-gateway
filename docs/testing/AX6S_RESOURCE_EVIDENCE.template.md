# AX6S V2-08 resource evidence (redacted template)

Copy this template for a staging run. Do not enter serial numbers, MAC/IP
addresses, node names, credentials, precise locations, raw traffic, or CA
material. Use broad firmware and memory/storage values where possible.

## Run metadata

- Date/time (coarse):
- Firmware family/version:
- Kernel family/version:
- Package version/architecture:
- Operator:
- Evidence bundle contains secrets: `no`

## Budget inputs

- Total memory / available memory bucket before install:
- Persistent free-space bucket before install:
- `/tmp` free-space bucket before install:
- Existing Gateway 1.7 configuration backed up: `yes/no`

## Measurements

| Scenario | Startup ms | Idle RSS KiB | Peak RSS KiB | CPU % | Persistent bytes | Log/cache bytes | Result |
|---|---:|---:|---:|---:|---:|---:|---|
| Disabled service | | | | | | | |
| One enabled profile | | | | | | | |
| Multiple enabled profiles | | | | | | | |
| Degraded/failed probe | | | | | | | |
| Restart/reload recovery | | | | | | | |
| Update success | | | | | | | |
| Health-failure rollback | | | | | | | |
| Interrupted-update recovery | | | | | | | |
| Low-space preflight | | | | | | | |

## Functional checks

- Gateway Wi-Fi Calling registration/traffic unaffected: `pass/fail`
- UDP 500/4500 untouched: `pass/fail`
- Only assigned device and exact Apple WLOC hosts were intercepted: `pass/fail`
- Ordinary HTTPS remained outside the WLOC data plane: `pass/fail`
- Stop/restart restored passthrough: `pass/fail`
- Configuration preserved across update/rollback: `pass/fail`
- No secrets or device identifiers in logs/support bundle: `pass/fail`

## Acceptance

- All ceilings in `resource-budget.conf` met: `yes/no`
- Budget exception approved by project lead: `yes/no/not applicable`
- AX6S result: `pass/fail/pending`
- Follow-up Issue:
