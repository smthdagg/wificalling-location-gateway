# V2 sing-box runtime provider

V2 does not install a second full-size sing-box binary. The unified Gateway
and WLOC supervisor resolves one executable from the following order:

1. `WLOC_SINGBOX_BIN` supplied by the service lifecycle;
2. `wificalling-gateway.main.singbox_bin` in UCI;
3. the first usable system provider: `sing-box-tiny`, `sing-box-lite`, a
   PassWall sing-box path, then the normal `/usr/bin/sing-box` fallback.

“Usable” means an absolute executable path that successfully answers
`version`. The selected binary is still started and supervised by this
project. Reusing the binary does not attach to or alter a PassWall-owned
process or configuration.

## AX6S acceptance

On the space-constrained test device, back up configuration and remove the old
Wificalling/WLOC application packages before installing the integrated package.
Keep the selected tiny/lite or PassWall provider installed. Do not remove it
as an application dependency and do not use forced dependency removal.

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
