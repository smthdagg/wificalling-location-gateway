# Wi‑Fi Calling + WLOC integrated product guide

This guide applies to the current independent repository product, which
contains and manages its own Wi‑Fi Calling Gateway and WLOC modules. It does
not install or depend on the separate Gateway 1.7 repository. For the concise
current workflow, also see [`docs/WLOC_TUTORIAL_EN.md`](WLOC_TUTORIAL_EN.md).

[中文教程](WIFICALLING_WLOC_TUTORIAL_ZH.md)

> Use this only on an authorized dedicated test device and network. WLOC does not change real GPS, cellular positioning, or the carrier emergency address. Never test emergency calls with a spoofed location.

# Part One: Wi‑Fi Calling Gateway

## 1. Preparation

Before starting, confirm that:

- The Wi‑Fi Calling & WLOC gateway and sing-box are installed and running.
- The test iPhone is connected to this router's Wi‑Fi.
- A proxy node in the SIM's home country or region is available.
- The iPhone has a usable LAN IPv4 address.
- Global proxies such as Passwall do not capture the test device.

Wi‑Fi Calling is sensitive to packet loss and jitter. Prefer a TCP-based node such as **AnyTLS, VLESS, VMess, or Trojan**. Hysteria2 and TUIC may establish a tunnel but can drop calls under public-network jitter.

## 2. Open WCG Setting

In LuCI, open **Services → WiFi Calling + WLOC Gateway → WCG Setting**.

![Wi‑Fi Calling settings](images/wificalling-wloc/01-wifi-calling-settings.png)

Under **General**:

1. Select **Enable**.
2. `Warning` is normally suitable for the log level.
3. Enable **Activity log** to record handshake and sustained encrypted-traffic metadata.
4. The default 60-second interval and 20 records per device are suitable for most tests.

## 3. Add and save a proxy node

Following the FAQ order, save a node before adding a device policy.

1. Paste one AnyTLS, Hysteria2/Hy2, TUIC, VLESS, VMess, Trojan, or WireGuard share link under **Import proxy node**, or select **Add proxy node** for manual entry.
2. Import parsing happens locally in the browser; the link is not sent to an external service.
3. Review the name, server, port, password or UUID, SNI, TLS, and protocol-specific fields.
4. Select **Save & Apply**.
5. If the node is missing from the device-policy selector, reload the page before continuing.

## 4. Add the iPhone device policy

Under **Device policies**:

1. Select **Add LAN device**.
2. Enter a recognizable device name.
3. Select **Independent tunnel**. **Follow gateway** does not use the plugin node.
4. Select the node saved in the previous step.
5. Enter the iPhone's current LAN IPv4 address. Alternatively use the **From connected devices** dropdown below: the plugin lists the LAN devices it detected (from the DHCP leases and the ARP cache, showing the real name and IP, excluding already-bound IPs and the router itself), and picking one fills in the device name and the IP automatically.
6. Select **Save & Apply** again.

![Proxy nodes and device policies](images/wificalling-wloc/02-device-policies.png)

Check the **DHCP binding** column:

- **Bound:** continue.
- **Pending** or **Device offline:** confirm that the iPhone is connected to this router.
- **MAC changed:** toggle iPhone Wi‑Fi so the gateway can rebind the current lease automatically.

The gateway identifies a device by IP. Traffic will not enter the selected node if the iPhone address differs from the policy.

## 5. Enable Wi‑Fi Calling on the iPhone

1. Open **Settings → Cellular**.
2. Select the SIM or line under test.
3. Open **Wi‑Fi Calling**.
4. Enable **Wi‑Fi Calling on This iPhone**.

Names vary by carrier and iOS version. If the option is absent, confirm that the SIM, plan, carrier, and region support Wi‑Fi Calling.

## 6. Check Wi‑Fi Calling Monitor & Log

Open **Wi‑Fi Calling Monitor & Log**.

![Wi‑Fi Calling monitor and activity log](images/wificalling-wloc/03-wifi-calling-monitor.png)

| Page state | Router-side network evidence |
|---|---|
| Not detected | No matching session observed |
| Negotiating | UDP 500 observed |
| NAT-T | UDP 4500 observed |
| Registered | Bidirectional UDP 4500 is `ASSURED` |
| Sustained traffic | Sustained bidirectional encrypted traffic after registration |

**Registered** is router-side network evidence, not carrier activation confirmation. The activity log records only handshake success, handshake failure, and sustained encrypted traffic. It cannot reveal phone numbers, SMS, voice content, or call direction.

If the carrier requires the location to match the SIM home region, the page may still show **Not detected**. Complete Part Two, wait a few minutes, and then refresh this page.

## 7. Wi‑Fi Calling troubleshooting

### It remains Not detected

Check node reachability, enabled device policy, Independent tunnel mode, the selected node, the iPhone IP and DHCP binding, and the iPhone Wi‑Fi Calling switch.

### It shows Registered but calls fail

Use a stable TCP-based node, confirm that the node country matches the SIM home region, and verify carrier activation for the line. Final validation requires an ordinary outgoing and incoming call.

### Airplane Mode check

Enable Airplane Mode, turn Wi‑Fi back on, and look for the carrier Wi‑Fi Calling indicator. This is a real-device record from the Wi‑Fi Calling Gateway project:

<img src="images/iphone/03-iphone-ee-wificall.jpg" alt="iPhone showing EE WiFiCall" width="420">

# Part Two: WLOC

## 8. Before configuring WLOC

Confirm that:

1. The proxy node and iPhone device policy from Part One have been saved.
2. The iPhone uses the LAN address in that device policy.
3. **Shadowrocket, Cloudflare WARP, Loon, WireGuard, and every other device VPN are off.** A device VPN bypasses the router redirect.
4. Safari will be used to download the certificate profile.

Router-side WLOC does not require a Cloudflare Worker, WLOC Plus website, TOKEN, Shadowrocket module, or on-device HTTPS decryption.

## 9. Get the WLOC certificate from the router

Open **WLOC Setting** and scroll to **Certificate (Safari install)**.

![Saved locations and certificate section](images/wificalling-wloc/07-wloc-saved-certificate.png)

1. Review the CA fingerprint, issue time, expiry time, and certificate status.
2. Select the displayed **Profile link**. The link is generated from the router's actual LAN IP (e.g. `http://192.168.31.1/wloc-ca.mobileconfig` for a 192.168.31.1 gateway, `http://192.168.1.1/wloc-ca.mobileconfig` for 192.168.1.1), so no manual editing is needed on any subnet.
3. **Regenerate profile** only re-exports the configuration profile.
4. Do not select **Generate new CA** unless intentional. A new CA invalidates previously installed profiles, and every test device must install and trust the new CA.

## 10. Download and install the profile on the iPhone

### 10.1 Open the URL in Safari

Enter the complete URL shown in WLOC Setting in Safari on the test iPhone.

![Enter the WLOC profile URL in Safari](images/iphone/01-wloc-profile-url.jpg)

### 10.2 Allow the download

When Safari says that the website is trying to download a configuration profile, select **Allow**.

![Allow Safari to download the configuration profile](images/iphone/02-wloc-profile-download.jpg)

### 10.3 Open Profile Downloaded

Open iPhone Settings and select **Profile Downloaded** near the top. If it is absent, go to **Settings → General → VPN & Device Management**.

![Open Profile Downloaded](images/iphone/03-wloc-profile-downloaded.jpg)

### 10.4 Install wloc-service root CA

Confirm that the profile and signer are `wloc-service root CA`, select **Install**, and complete the iOS prompts.

![Install the wloc-service root CA profile](images/iphone/04-wloc-profile-install.jpg)

## 11. Enable full certificate trust

After installing the profile:

1. Open **Settings → General → About → Certificate Trust Settings**.
2. Find `wloc-service root CA`.
3. Enable full trust and confirm the warning.

![Enable full trust for wloc-service root CA](images/iphone/05-wloc-certificate-trust.jpg)

Back in WLOC Setting, the fingerprint shown in the iPhone profile may be pasted under **Verify iPhone certificate**. Select **Verify** and confirm that it matches the router CA.

## 12. Configure Auto or Manual location

Return to **WLOC Setting**.

![WLOC module, location mode, and manual search](images/wificalling-wloc/04-wloc-settings.png)

### 12.1 Select the followed device

Under **Follow device**, select the test iPhone configured in Part One. Auto mode follows the exit of the node bound to this device.

### 12.2 Auto mode

1. Set **Location mode** to **Auto (follow node)**.
2. Confirm the followed device.
3. Enable **WLOC interception**.
4. Select **Save & Apply**.

Auto mode uses the node exit IP to determine country, city, timezone, and coordinates. After changing the bound node, wait for a new exit probe and target update.

### 12.3 Manual mode

1. Enter a place such as `London, UK` under **Place name**, then select **Search**.
2. Search returns a result but does not change the active location.
3. Review latitude and longitude, then select **Apply coordinates**.
4. Coordinates may also be entered directly before selecting **Apply coordinates**.
5. Select **Apply** beside a saved location, or use **Add saved location** to create a preset.
6. Finish with **Save & Apply**.

Manual coordinates and presets are stored in `/etc/config/wloc-service` and survive a reboot. GPS values remain in the router's local administration plane.

## 13. Re-trigger location on the iPhone

After switching mode or target, use one of the FAQ methods:

1. Toggle Airplane Mode once; or
2. Toggle Wi‑Fi; or
3. Force-close and reopen Maps, Weather, or the app under test.

Location requests are triggered by iPhone apps, and Apple location services may cache results. Wait briefly and trigger again if the first attempt does not refresh.

## 14. Check WLOC Status & Logs

Open **WLOC Status & Logs**.

![WLOC current location and usage log](images/wificalling-wloc/05-wloc-monitor.png)

Check that:

- `Service phase` is `intercepting`.
- `Follow device` is the test iPhone. The **Refresh IP** button next to it re-probes the followed node's exit IP immediately.
- After you switch a device's node in the Wi‑Fi Calling settings, the monitor exit IP follows the new node automatically within about 10 seconds; click **Refresh IP** to make it immediate.
- `Location mode` matches Auto or Manual.
- Country, city, timezone, and coordinates match the target.
- `Geo state` is `fresh`.
- The usage log contains a **Target updated** event.

The usage log retains the newest 20 records and can be cleared. It records target-update time, place, and auto/manual source only; raw WLOC responses are never logged.

**Target updated** means only that the router updated its target. It does not prove that real GPS, cellular positioning, or the carrier emergency address changed.

## 15. WLOC troubleshooting

### Location does not change

Confirm that the profile is installed, `wloc-service root CA` has full trust, interception is enabled, the correct device is selected, and all device VPNs are off. Re-trigger Maps or Weather.

### Auto location does not match the node

Confirm that the Part One device policy is bound to the intended node, global proxies such as Passwall bypass the test device, and no device VPN is running. After a node switch the monitor exit IP follows within about 10 seconds, or click **Refresh IP** for an immediate re-probe; wait for `Geo state` to become `fresh`.

### Certificate verification fails

Compare the fingerprint in the iPhone profile with the router CA fingerprint. If they differ, remove the old iPhone profile, download it again from the current router, install it, and enable full trust.

### Restore the original location

Disable **WLOC interception** and select **Save & Apply**. To leave the test completely, remove the WLOC profile from the iPhone and disable its root trust.

## 16. Final check

After WLOC status is correct, return to **Wi‑Fi Calling Monitor & Log** and wait a few minutes. When **Registered** appears, complete one ordinary outgoing and incoming call.

![Built-in FAQ](images/wificalling-wloc/06-help-faq.png)

## 17. Security and privacy

- Never publish node links, passwords, UUIDs, private keys, CA private keys, or complete certificate fingerprints.
- Do not commit real device identifiers, raw traffic, or precise personal locations to GitHub.
- Trust the WLOC CA only on a dedicated test iPhone and remove it after testing.
- WLOC must remain limited to Apple WLOC traffic from the assigned device; normal HTTPS sites must not present a wloc-service certificate.
- Follow local law, Apple terms, and carrier terms. Emergency services must always use the real location.

## Sources

- [Wi‑Fi Calling Gateway English README](https://github.com/smthdagg/luci-app-wificalling-gateway/blob/main/README_EN.md)
- This project's current LuCI pages, built-in FAQ, and AX6S real-device validation notes
