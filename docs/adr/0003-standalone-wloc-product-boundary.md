# ADR 0003: standalone WLOC product boundary

Status: Historical; superseded by ADR 0004 (2026-08-22)

## Decision

This ADR records the earlier standalone-WLOC interpretation. It is retained to
explain the design transition, but it is not the current product boundary.
ADR 0004 defines `wificalling-location-gateway` as one independent project that
contains both WiFi Calling Gateway and WLOC and explicitly replaces this
decision.

The product owns one lifecycle, one configuration model, one LuCI application,
one status/monitoring model, one bounded log model, and one transactional update
boundary. Its only optional runtime provider is a locally installed sing-box
binary (including the AX6S-tested tiny/lite variant or a PassWall-provided
binary). A provider is an executable capability, not a dependency on PassWall
or on another application’s UCI schema or service lifecycle.

The former `wificalling-gateway` init script, UCI file, menu, and package
metadata are legacy integration history. They are not shipped by the current
standalone payload and must not be required for a clean installation. The
repository/package name remains only as a compatibility identity for existing
feeds and upgrade paths.

## Configuration model

`/etc/config/wloc-service` is authoritative. It contains an independent WLOC
node catalog and device profiles. The product must not read
`/etc/config/wificalling-gateway` to discover devices, nodes, or locations.

Each device profile contains:

- LAN identity and display name;
- WLOC service enablement;
- an explicit WLOC node reference;
- location mode: `auto` or `manual`;
- manual latitude/longitude or a saved location reference;
- bounded per-device health, monitoring, and log settings.

`fixed` means “use the explicit WLOC node selected by this profile.” There is
no ambiguous `gateway_default` or “follow Gateway” mode in the standalone
product. `auto` means that this profile’s location follows the exit of its own
selected WLOC node. `manual` means that this same profile uses its own stored
coordinates. A manual setting is therefore written to the device profile, not
to a global WLOC singleton.

## Management surface

The LuCI information architecture is:

1. Overview — aggregate health and resource summary;
2. Basic Settings — global service/provider/log defaults only;
3. Devices — one row/card per device, including node, auto/manual location,
   enablement, health, monitor, and device log entry point;
4. Nodes / Provider — WLOC-owned node references and sing-box provider status;
5. Logs & Monitoring — unified bounded observation and per-device filtering;
6. Component Update — an independent page, not embedded in health;
7. Help.

There is no separate global WLOC location page that duplicates device profile
fields. Certificate and advanced protocol controls may remain separate only
when they are not device-specific.

## Update compatibility

The independent Component Update page must preflight the actual router before
installation. It checks package format, architecture, OpenWrt release family,
package-manager capability, required kernel/module capabilities, free space,
and the product/API major version. It must not check or require a Gateway
version. A package built for another architecture, firmware family, or package
format is rejected before `opkg`/`apk` runs.

## Language policy

All UI and RPC user-facing messages are English source strings first. Chinese
is delivered through the project’s LuCI language package. No new page may use
hard-coded Chinese strings or a parallel ad-hoc translation table as its source
of truth. Existing front-end translation shims are migration debt and must be
replaced or reduced to a compatibility bridge during the UI rewrite.

## Consequences

- The Gateway 1.7.3 artifact is not a release input, runtime dependency, or
  AX6S acceptance prerequisite for this project.
- Existing AX6S Gateway configuration is not part of WLOC migration. WLOC
  migration backs up and converts only its own configuration and CA state.
- The sing-box provider contract must be small, detected at runtime, and fully
  documented; provider absence produces a safe disabled/passthrough state.
- Current code paths that import Gateway UCI, install Gateway payloads, expose
  Gateway pages, or require Gateway compatibility metadata must be removed or
  explicitly isolated behind a non-default migration tool before release.
