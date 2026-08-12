# AX6S deployment guide (first real-device test)

Target: Redmi AX6S (mt7622, AArch64) running OpenWrt 24.10, with the Wi-Fi
Calling Gateway 1.7 and sing-box already installed and configured.

## 1. Produce the AArch64 binary

```sh
OPENWRT_BIN_NAME=wloc-service \
OPENWRT_CROSS_CACHE_DIR=/tmp/wloc-rust-openwrt-deploy \
./scripts/ci/verify-rust-openwrt.sh
```

The output is a static, stripped `aarch64-unknown-linux-musl` ELF (verified
~1.7 MiB) at `<cache>/output/wloc-service`.

## 2. Install on the router

```sh
scp /tmp/wloc-rust-openwrt-deploy/output/wloc-service root@AX6S:/usr/sbin/wloc-service
ssh root@AX6S 'chmod 0755 /usr/sbin/wloc-service'
# optional: install the procd init script + UCI config from openwrt/files/
scp openwrt/files/etc/init.d/wloc-service root@AX6S:/etc/init.d/wloc-service
ssh root@AX6S 'chmod 0755 /etc/init.d/wloc-service'
```

## 3. Start the daemon (first run)

```sh
ssh root@AX6S '
  mkdir -p /etc/wloc-service
  WLOC_SOCKET=/var/run/wloc-service/control.sock \
  WLOC_CA_CERT=/etc/wloc-service/ca.pem \
  WLOC_CA_KEY=/etc/wloc-service/ca.key \
  WLOC_PROXY_PORT=8443 \
  /usr/sbin/wloc-service &
'
```

First run generates the root CA, persists the private key in
`/etc/wloc-service/ca.key` (mode 0600) and exports `ca.pem`. Restarts load
the same CA so iPhone trust survives.

## 4. Install the CA on the iPhone

Copy `/etc/wloc-service/ca.pem` to the iPhone (AirDrop / config profile /
email), install it in Settings → General → About → Certificate Trust
Settings, and **enable full trust** for the "wloc-service root CA".

> Do this only on your dedicated test iPhone. The CA can impersonate the two
> approved Apple hosts for that device; keep it private to your lab.

## 5. Pin the test device and install the redirect

Give the test iPhone a fixed IP via the Gateway 1.7 DHCP static lease (or
dnsmasq reservation), then on the router:

```sh
scp scripts/openwrt/wloc-redirect.sh root@AX6S:/usr/sbin/wloc-redirect.sh
ssh root@AX6S '
  chmod 0755 /usr/sbin/wloc-redirect.sh
  WLOC_DEVICE_IP=<TEST_IPHONE_IP> /usr/sbin/wloc-redirect.sh install
'
```

The redirect lives in its own `wloc_service` nftables table and only maps the
test device's TCP 443 traffic to `gs-loc.apple.com` / `gs-loc-cn.apple.com`
to the local proxy port. No other traffic is touched.

## 6. Phase 6 validation sequence

1. Back up the Gateway 1.7 config.
2. Verify the iPhone trust profile fingerprint matches `ca.pem`.
3. Ensure Shadowrocket is NOT running.
4. Select the UK node on the Gateway; confirm the exit IP / city.
5. Trigger Apple network location (Wi-Fi + Maps / Settings), confirm the
   WLOC service reports the patched country.
6. Switch US/HK nodes; confirm location follows.
7. In Safari, open a normal HTTPS site: its certificate must NOT be issued by
   the wloc-service CA (proves interception is limited).
8. Confirm UDP 500/4500 (Wi-Fi Calling tunnel) and Gateway functionality are
   unaffected.

## 7. Inspect and roll back

- Control API: `socat - UNIX-CONNECT:/var/run/wloc-service/control.sock` and
  send a `status.get` frame, or use the daemon logs.
- Remove the redirect: `WLOC_DEVICE_IP=<IP> /usr/sbin/wloc-redirect.sh remove`.
- Stop the daemon: kill the process; sockets/table entries from the daemon's
  own table are cleaned by the redirect removal and a daemon restart always
  re-loads the persisted CA.

## Notes and limits

- The exit probe is currently a stub (`WLOC_STUB_EXIT_IP`, default 8.8.8.8 →
  US); until the real sing-box probe lands, the patched location follows the
  stub, not the selected node. Wire the real probe before relying on
  UK/US/HK switching.
- The Geo provider is live (ip-api.com); `WLOC_GEO_PROVIDER=stub` forces the
  deterministic stub.
- IPv6: the redirect is IPv4-only. Suppress AAAA for the two WLOC hosts on
  the test device (Gateway 1.7 handles device policy) so the iPhone uses the
  IPv4 path.
