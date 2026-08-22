# ADR 0004: integrated WiFi Calling Gateway and WLOC product boundary

Status: accepted for v2.0 implementation

## Decision

`wificalling-location-gateway` is one independent OpenWrt project containing
both the WiFi Calling Gateway and the Apple WLOC location service. The project
does not depend on, modify, vendor, or install the separate Wi-Fi Calling
Gateway 1.7 repository. “Independent” describes the repository and release
boundary; it does not mean that the Gateway module is removed from this
product.

The package owns both UCI configurations, both init/data-plane modules, the
shared supervisor, one LuCI management surface, and the common diagnostics,
component update, rollback, and resource-budget policy. The two modules keep
their own protocol and firewall scopes, but they are started, stopped,
observed, and updated as one product lifecycle.

## Consequences

- The AX6S package must include Gateway and WLOC payloads and both conffiles.
- Gateway node configuration remains available; WLOC device profiles select
  their WLOC node and location mode independently.
- WLOC may only manage its profile-scoped Apple TCP/443 redirect tables. It
  must not intercept UDP 500/4500 or mutate the Gateway nftables namespace.
- The shared supervisor may reuse a Gateway-generated sing-box configuration
  for WLOC when both modules are enabled. If only WLOC is enabled, it may use
  the configured tiny/lite or PassWall provider.
- The separate Gateway 1.7 repository is an external historical project, not
  a runtime or packaging dependency.

## Supersession

ADR 0003 documented an earlier standalone-WLOC boundary and is retained as
historical evidence. It is superseded for the v2 product by this ADR.
