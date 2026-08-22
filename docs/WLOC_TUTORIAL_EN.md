# WiFi Calling + WLOC Gateway tutorial

This is the v2 guide for the independent integrated project. It configures
the in-repository WiFi Calling Gateway and WLOC modules together; it does not
install or depend on the separate Gateway 1.7 repository.

1. Install the matching OpenWrt package and retain the sing-box tiny/lite or
   PassWall provider.
2. Open Services → WiFi Calling + WLOC Gateway → WCG Setting. Add
   nodes and Gateway device routes as required by the local network.
3. Open WLOC Setting. Enable WLOC, choose the Geo provider, and
   confirm the provider configuration path. When Gateway is enabled, WLOC
   reuses the Gateway-generated sing-box configuration.
4. Open WLOC Devices and create one WLOC profile per authorized LAN device. Enter its
   private address, explicit WLOC node reference, and enable the profile.
5. Choose Auto follow selected node or Manual location. Manual latitude and
   longitude are stored on that same device profile.
6. Apply the change and confirm WCG Status & Logs, WLOC Setting, and WCG WLOC
   Service Monitor show the child daemons, provider, redirect, and profile as healthy.
7. Install and trust the local CA only on the authorized test iPhone. Trigger
   Maps or Weather and inspect WLOC Status & Logs.
8. Use Component Update separately. Stage a signed local package, run
   preflight, then apply. If LuCI fails, use the manual SSH commands shown on
   that page. Recover an interrupted transaction before starting a second update.

For AX6S, remove the previous WLOC package before installation to conserve
overlay space. Never remove the selected provider binary.
