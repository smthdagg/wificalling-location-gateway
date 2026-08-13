# Wi‑Fi Calling & WLOC Complete User Guide

This guide applies to an OpenWrt or ImmortalWrt gateway that combines Wi‑Fi Calling Gateway 1.7 with the router-side WLOC service. It starts after the software has been installed and the LuCI pages are available.

[中文教程](WIFICALLING_WLOC_TUTORIAL_ZH.md)

## 1. What this project does

The project contains two side-by-side modules:

- **Wi‑Fi Calling Gateway** routes selected LAN devices through selected sing-box nodes and reports observable ePDG/IPsec UDP 500/4500 sessions.
- **WLOC** handles Apple network-location requests for the assigned test iPhone. It can follow the selected node's exit location or use a manually selected location.

The modules share nodes and device policies but have different responsibilities. Router-side WLOC does not require a Cloudflare Worker, the WLOC Plus dashboard, a TOKEN, a Shadowrocket module, or on-device HTTPS decryption. Shadowrocket, Cloudflare WARP, Loon, WireGuard, and other device VPNs must be off during testing so that traffic continues through the router.

> Use this only with a dedicated, authorized test device. WLOC does not replace GPS, cellular positioning, or a carrier emergency address. Never test emergency calls with a spoofed location.

## 2. Prerequisites

1. The Wi‑Fi Calling & WLOC gateway is installed and running on the router.
2. sing-box works, with an exit node in the SIM's home country or region.
3. The test iPhone is connected to this router's Wi‑Fi.
4. The iPhone has a stable LAN IPv4 address. The gateway maintains the DHCP static binding used by its device policy.
5. All on-device VPNs, proxies, and global tunnels are disabled.

Prefer a TCP-based node such as **AnyTLS, VLESS, VMess, or Trojan**. Hysteria2 and TUIC may show a Wi‑Fi Calling indicator but can drop calls under public-network jitter. WireGuard can work but should still be tested for stability.

## 3. Configure Wi‑Fi Calling Gateway

Open LuCI and go to **Services → WifiCalling&Wloc Gateway → Wi‑Fi Calling Settings**.

![Wi‑Fi Calling settings](images/wificalling-wloc/01-wifi-calling-settings.png)

### 3.1 General settings

1. Select **Enable**.
2. `Warning` is normally suitable for the log level.
3. Enable **Activity log** if handshake and sustained encrypted-traffic records are required.
4. The default 60-second activity interval and 20 records per device are suitable for most tests.

### 3.2 Add a proxy node

Select **Add proxy node** for manual entry, or paste one AnyTLS, Hysteria2/Hy2, TUIC, VLESS, VMess, Trojan, or WireGuard share link under **Import proxy node**.

Import parsing happens locally in the browser. Review the server, port, password or UUID, SNI, TLS, Reality, WebSocket, and other protocol fields, then select **Save & Apply**. A node must be saved before it can be selected in a device policy.

### 3.3 Add the iPhone device policy

Under **Device policies**, select **Add LAN device** and enter:

1. A recognizable device name.
2. **Independent tunnel** as the routing mode.
3. The previously saved exit node.
4. The iPhone's current LAN IPv4 address.

Select **Save & Apply** again. The **DHCP binding** column should show **Bound**. If it shows pending, changed MAC, or offline, toggle iPhone Wi‑Fi so it obtains the reserved address, then check again.

![Proxy nodes and device policies](images/wificalling-wloc/02-device-policies.png)

## 4. Enable Wi‑Fi Calling on the iPhone

1. Open **Settings → Cellular**.
2. Select the relevant SIM or line.
3. Open **Wi‑Fi Calling**.
4. Enable **Wi‑Fi Calling on This iPhone**.

Names vary by carrier and iOS version. If the option is absent, confirm that the SIM, plan, carrier, and region support Wi‑Fi Calling.

## 5. Install the WLOC root CA on the iPhone

The router generates the WLOC certificate. This project does not use a Shadowrocket certificate.

### 5.1 Download and install the profile

1. In Safari on the test iPhone, open `http://192.168.31.1/wloc-ca.mobileconfig`. Replace `192.168.31.1` if the router uses another LAN address.
2. After Safari reports that the profile was downloaded, open iPhone Settings.
3. Select **Profile Downloaded** near the top, or go to **Settings → General → VPN & Device Management**.
4. Select the WLOC profile and complete installation.

The following iOS system screenshot is reused from the iOS Location Spoofer Plus guide because the menu path is identical. In this project the installed profile should be named **wloc-service**, not Shadowrocket.

![iOS VPN & Device Management](images/iphone/01-ios-vpn-device-management.jpg)

### 5.2 Enable full trust

1. Open **Settings → General → About → Certificate Trust Settings**.
2. Find **wloc-service root CA**.
3. Enable full trust and confirm the iOS warning.

![iOS Certificate Trust Settings](images/iphone/02-ios-certificate-trust.jpg)

Install the CA only on the dedicated test iPhone. Remove the profile and its trust when testing is complete.

## 6. Configure WLOC

Open **Services → WifiCalling&Wloc Gateway → WLOC Settings**.

![WLOC settings](images/wificalling-wloc/04-wloc-settings.png)

### 6.1 Select the followed device

Under **Follow device**, select the test iPhone configured earlier. Auto mode uses the node bound to this device and derives the WLOC target from that node's exit IP.

### 6.2 Auto mode

1. Set **Location mode** to **Auto (follow node)**.
2. Confirm that the test device is bound to the intended node.
3. Enable **WLOC interception**.
4. Select **Save & Apply**.

When the device is moved to another node, WLOC updates its target on the next refresh.

### 6.3 Manual mode

To use a fixed location:

1. Enter a place such as `London, UK` under **Manual search**, then select **Search**.
2. Review the returned city, latitude, and longitude.
3. Select **Apply coordinates**. Searching without applying does not change the location.
4. Coordinates may also be entered directly, or a saved location may be applied.
5. Applying coordinates selects manual mode and stores the values in the router's local configuration.

## 7. Refresh location on the iPhone

1. Confirm that Shadowrocket, WARP, and every other device VPN are off.
2. Open **Settings → Privacy & Security → Location Services**.
3. Turn Location Services off, wait about 5–10 seconds, and turn it on again.
4. Force-close Maps, Weather, or the app under test, then reopen it.
5. If the location still does not refresh, toggle Airplane Mode once or toggle Wi‑Fi, then wait briefly.

Apple location services can cache results. In auto mode, the public IP, WLOC target, and the node bound to the device should point to the same country or region.

## 8. Verify WLOC

Open **WLOC Monitor & Log**.

![WLOC current location and log](images/wificalling-wloc/05-wloc-monitor.png)

Check that:

- `Service phase` is `intercepting`.
- `Follow device` is the test iPhone.
- `Location mode` matches Auto or Manual.
- Country, city, timezone, and coordinates match the target.
- `Geo state` is `fresh`.
- The usage log contains a **Target updated** event.

**Target updated** proves only that the router updated its WLOC target. It does not prove that GPS, cellular positioning, or the carrier emergency address changed.

## 9. Verify Wi‑Fi Calling

Open **Wi‑Fi Calling Monitor & Log**.

![Wi‑Fi Calling monitor and activity log](images/wificalling-wloc/03-wifi-calling-monitor.png)

The normal observation sequence is:

1. UDP 500 appears: negotiation started.
2. UDP 4500 appears: NAT-T was observed.
3. UDP 4500 becomes bidirectional and `ASSURED`: the page shows **Registered**.
4. Sustained bidirectional encrypted traffic after registration may be shown as **Call in progress (inferred from sustained encrypted traffic)**.

**Registered** is router-side network evidence, not carrier activation confirmation. Complete an ordinary authorized outgoing and incoming call test. The router cannot see phone numbers, SMS, voice content, or call direction inside the encrypted tunnel.

You may also enable Airplane Mode and then turn Wi‑Fi back on to check for the carrier Wi‑Fi Calling indicator. This is a real-device record from the Wi‑Fi Calling Gateway project:

<img src="images/iphone/03-iphone-ee-wificall.jpg" alt="iPhone showing EE WiFiCall" width="420">

## 10. Troubleshooting

### Wi‑Fi Calling remains Not detected

Check node reachability, Independent tunnel mode, the iPhone address, DHCP binding, and the iPhone Wi‑Fi Calling switch.

### Registered appears but calls fail

Registered only proves an ASSURED UDP 4500 flow. Prefer a stable TCP-based node, confirm that the node location matches the SIM home region, and verify carrier activation for the line.

### WLOC does not change

Confirm that the wloc-service profile is installed and fully trusted, WLOC interception is enabled, the correct device is selected, and all device VPNs are off. Toggle Location Services and reopen Maps or Weather.

### Auto location does not match the node

Confirm the device is bound to the intended node, no device VPN is active, global proxies such as Passwall are not capturing that device, and Geo state has had time to refresh.

### Restore the original location immediately

Disable **WLOC interception**, then save and apply. Apple WLOC traffic should return to its original response. To leave the test completely, remove the WLOC profile and root trust from the iPhone.

![Built-in help](images/wificalling-wloc/06-help-faq.png)

## 11. Security notes

- Never publish proxy share links, passwords, UUIDs, private keys, or CA private keys.
- Do not commit real device identifiers, raw traffic, or precise personal locations to GitHub.
- Trust the CA only on an authorized test iPhone.
- WLOC must remain limited to the assigned device's Apple WLOC traffic and must not intercept normal HTTPS sites.
- Follow local law, Apple terms, and carrier terms, and always use the real location for emergency services.

## Sources

- [Wi‑Fi Calling Gateway English README](https://github.com/smthdagg/luci-app-wificalling-gateway/blob/main/README_EN.md)
- [iOS Location Spoofer Plus complete installation guide](https://github.com/smthdagg/ios-location-spoofer-plus/blob/main/%E5%AE%8C%E6%95%B4%E7%9A%84%E5%AE%89%E8%A3%85%E4%BD%BF%E7%94%A8%E6%95%99%E7%A8%8B.md)
- This project's current LuCI pages, built-in FAQ, and AX6S real-device validation notes
