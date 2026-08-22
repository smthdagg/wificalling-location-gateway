# V2 sing-box runtime provider

V2 does not install a second full-size sing-box binary. The integrated
Gateway/WLOC supervisor resolves one executable from the following order:

1. `WLOC_SINGBOX_BIN` supplied by the service lifecycle;
2. `wloc-service.main.singbox_bin` in the WLOC UCI section;
3. the first usable system provider: `sing-box-tiny`, `sing-box-lite`, a
   PassWall sing-box path, then the normal `/usr/bin/sing-box` fallback.

“Usable” means an absolute executable path that successfully answers
`version`. The selected binary is started and supervised by the integrated
Gateway/WLOC lifecycle. When Gateway is enabled, WLOC reuses the Gateway
generated configuration. Reusing a PassWall binary does not attach to or alter
a PassWall-owned process or configuration.

## AX6S acceptance

On the space-constrained test device, back up configuration and remove the old
WLOC application package before installing the standalone package. Keep the
selected tiny/lite or PassWall provider installed. Do not remove it as an
application dependency and do not use forced dependency removal.

After installation, verify the provider before enabling traffic interception:

```sh
/usr/libexec/wificalling-location-gateway/singbox-runtime.sh path
/usr/libexec/wificalling-location-gateway/singbox-runtime.sh version
/etc/init.d/wificalling-location-gateway restart
/usr/sbin/wloc-health.sh
```

If the resolver reports no provider, install or enable the already-tested
sing-box tiny/lite package or the PassWall-provided binary, then restart the
unified service. The package post-install warning is actionable and does not
silently download or duplicate a large binary.

The release evidence must record only the provider class/path category and
coarse RSS/storage values; do not publish node credentials, exact device
identifiers, or raw traffic.
