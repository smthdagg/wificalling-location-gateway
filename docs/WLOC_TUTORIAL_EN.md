# WLOC Location Service tutorial

This is the V2 standalone WLOC guide. It does not configure Wi-Fi Calling
Gateway.

1. Install the matching OpenWrt package and retain the sing-box tiny/lite or
   PassWall provider.
2. Open Services → WLOC Location Service → Basic Settings. Enable WLOC,
   choose the Geo provider, and confirm the provider configuration path.
3. Open Devices and create one profile per authorized LAN device. Enter its
   private address, explicit WLOC node reference, and enable the profile.
4. Choose Auto follow selected node or Manual location. Manual latitude and
   longitude are stored on that same device profile.
5. Apply the change and confirm Overview/Service Status show the daemon,
   provider, redirect, and profile as healthy.
6. Install and trust the local CA only on the authorized test iPhone. Trigger
   Maps or Weather and inspect Logs & Monitoring.
7. Use Component Update separately. Stage a signed local package, run
   preflight, then apply. Recover an interrupted transaction before starting a
   second update.

For AX6S, remove the previous WLOC package before installation to conserve
overlay space. Never remove the selected provider binary.

