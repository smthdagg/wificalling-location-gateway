# Wi‑Fi Calling Gateway + WLOC 完整教程

**中文 / English · Redmi AX6S · ImmortalWrt 24.10 · Wi‑Fi Calling Gateway 1.7**

> 本教程面向专用测试路由器和专用测试 iPhone。WLOC 修改的是 Apple 网络定位响应，不是手机 GPS，也不是运营商的紧急呼叫定位。请先阅读“安全、隐私与合规”章节。
>
> This guide is for a dedicated test router and a dedicated test iPhone. WLOC changes an Apple network-location response; it does not control GPS and does not certify carrier or emergency-call location. Read “Security, privacy and compliance” before testing.

## 1. 你将完成什么 / What you will build

Wi‑Fi Calling Gateway 1.7 and WLOC are two coordinated data planes in one independent OpenWrt package:

```text
iPhone
  ├─ Wi‑Fi Calling: UDP 500/4500 → existing Gateway 1.7 → ePDG
  └─ Apple WLOC: TCP 443 + two exact hostnames → WLOC service → verified Apple upstream

Gateway node binding → sing-box exit probe → Geo result → WLOC target location
```

The gateway can follow the exit location of the node bound to the test device, or use a manually selected city/coordinate. The interception scope is deliberately narrow:

- one assigned test device;
- `gs-loc.apple.com` and `gs-loc-cn.apple.com` only;
- TCP port 443 only;
- a dedicated WLOC ruleset, never the Gateway 1.7 IPsec/Wi‑Fi Calling table.

网关可以跟随测试设备绑定节点的出口位置，也可以使用手动城市/坐标。拦截范围严格限制为：一台测试设备、两个 Apple WLOC 域名、TCP 443，以及独立 WLOC 规则集；不会修改 Gateway 1.7 的 IPsec/Wi‑Fi Calling 规则。

## 2. 前置条件 / Prerequisites

### Hardware and firmware / 硬件与固件

- Redmi AX6S or a comparable AArch64 OpenWrt router.
- ImmortalWrt/OpenWrt 24.10 with enough `/tmp` space for a test build.
- Wi‑Fi Calling Gateway 1.7 and sing-box already installed.
- One dedicated iPhone for testing; reserve its LAN address in DHCP.
- A Mac/Linux build host with Git, Rust 1.90, Docker and the OpenWrt toolchain helper.

### Network and account preparation / 网络与账户准备

1. Make sure the router can reach the selected sing-box node.
2. Keep the iPhone on the router Wi‑Fi during the test.
3. Disable Cloudflare WARP, Shadowrocket, Loon, WireGuard and every other iPhone VPN while testing. A device VPN can bypass the router path.
4. Prepare a rollback owner and a short test window. Do not test emergency calls with a spoofed location.

## 3. 安装与构建 / Build and install

The repository contains a reproducible OpenWrt build helper and package scaffold. Build from the WLOC project checkout, not from the stable Gateway 1.7 source tree.

仓库提供可复现的 OpenWrt 构建脚本和打包骨架。请在 WLOC 项目目录构建，不要修改稳定的 Gateway 1.7 源码仓库。

### Option A: reproducible AArch64 binary / 方式 A：可复现 AArch64 二进制

```sh
OPENWRT_BIN_NAME=wloc-service \
OPENWRT_CROSS_CACHE_DIR=/tmp/wloc-rust-openwrt-deploy \
./scripts/ci/verify-rust-openwrt.sh
```

The output is a stripped static AArch64 binary under the cache output directory. Copy it together with the init script and UCI skeleton:

```sh
scp /tmp/wloc-rust-openwrt-deploy/output/wloc-service root@AX6S:/usr/sbin/
scp openwrt/files/etc/init.d/wloc-service root@AX6S:/etc/init.d/
scp openwrt/files/etc/config/wloc-service root@AX6S:/etc/config/
ssh root@AX6S 'chmod 0755 /usr/sbin/wloc-service /etc/init.d/wloc-service'
```

### Option B: OpenWrt package / 方式 B：OpenWrt IPK

On a prepared OpenWrt build tree, build `wloc-service` and `wloc-ctl` with the package Makefile. Install the resulting IPK on the router, then start the service with procd. Keep the IPK, commit and toolchain digest alongside the deployment record.

在准备好的 OpenWrt 构建树中使用 package Makefile 构建 `wloc-service` 和 `wloc-ctl`。将生成的 IPK 安装到路由器，再由 procd 启动服务。请保存 IPK、提交号和工具链摘要，便于回滚。

```sh
opkg install ./wloc-service_*.ipk ./wloc-ctl_*.ipk
/etc/init.d/wloc-service enable
/etc/init.d/wloc-service start
```

> 如果你的构建产物尚未包含 LuCI package，请先完成 daemon 的验证，再安装独立 LuCI 包。不要把未通过 CI 的临时二进制放进生产路由器。
>
> If your build does not include the LuCI package yet, validate the daemon first and install the independent LuCI package afterward. Never deploy an unverified temporary binary.

## 4. 首次启动与证书 / First start and CA trust

The daemon uses a root-owned Unix control socket and a router-local CA. The private key must stay on the router with mode `0600`.

服务使用 root 专属 Unix socket 和路由器本地 CA。私钥必须留在路由器上，权限为 `0600`。

```sh
ssh root@AX6S '
  mkdir -p /etc/wloc-service
  chmod 0700 /etc/wloc-service
  /etc/init.d/wloc-service restart
  ls -l /etc/wloc-service/ca.key /etc/wloc-service/ca.pem
'
```

Install the public profile on the test iPhone:

在测试 iPhone 上安装公开证书描述文件：

1. Open the LuCI WLOC page and download the CA/mobileconfig link.
2. Open the link in Safari on the iPhone.
3. Install the profile under **Settings → General → VPN & Device Management**.
4. Enable full trust under **Settings → General → About → Certificate Trust Settings**.
5. Compare the displayed SHA‑256 fingerprint with the fingerprint shown by LuCI.

Do not email, upload or commit `ca.key`. The WLOC CA can impersonate the two approved Apple hosts for the test device; treat it as a lab secret.

不要发送、上传或提交 `ca.key`。WLOC CA 可以在测试设备上代表两个允许的 Apple 域名，因此必须按实验室机密管理。

## 5. 配置 Wi‑Fi Calling Gateway / Configure Wi‑Fi Calling

Open **Services → WifiCalling&Wloc Gateway → Wi‑Fi Calling Settings**.

打开 **Services → WifiCalling&Wloc Gateway → Wi‑Fi Calling Settings**。

![Wi‑Fi Calling settings](images/wificalling-wloc/01-wifi-calling-settings.png)

1. Enable the Gateway and choose an appropriate log level.
2. Add or import a proxy node. Node links are parsed locally in the browser; never paste a link into an untrusted service.
3. Under **Device policies**, add the iPhone and reserve its LAN IPv4 address.
4. Bind the device to the node you want WLOC auto mode to follow.
5. Click **Save & Apply**, then wait for the node health check.

设备策略示例：

![Device policies](images/wificalling-wloc/02-device-policies.png)

### Wi‑Fi Calling verification / Wi‑Fi Calling 验证

On the iPhone, join the router Wi‑Fi and enable **Settings → Cellular → Wi‑Fi Calling**. Open the monitor page:

在 iPhone 上连接路由器 Wi‑Fi，并开启 **设置 → 蜂窝网络 → Wi‑Fi 通话**。然后打开监控页：

![Wi‑Fi Calling monitor](images/wificalling-wloc/03-wifi-calling-monitor.png)

“Registered” means the router observed the expected bidirectional UDP 4500 tunnel. It is network evidence, not proof of carrier activation. Confirm the service with an ordinary test call only when your carrier and test plan permit it.

“Registered” 表示路由器观察到预期的双向 UDP 4500 隧道。这只是网络层证据，不等于运营商已确认激活。只有在运营商条款和测试计划允许时，才进行普通呼叫验证。

## 6. 配置 WLOC / Configure WLOC

Open **WLOC Settings**.

打开 **WLOC Settings**。

![WLOC settings](images/wificalling-wloc/04-wloc-settings.png)

### Enable the module / 开启模块

Turn on **Enable WLOC interception** and save. The service should move through `starting`/`ready_passthrough` before it reaches `intercepting`. If health, scope, IPv6 readiness or watchdog checks fail, it must remain pass-through or disabled.

开启 **Enable WLOC interception** 并保存。服务应依次经过 `starting` / `ready_passthrough`，再进入 `intercepting`。如果健康检查、流量范围、IPv6 或 watchdog 检查失败，服务必须保持透传或禁用。

### Auto mode / 自动模式

Select **Auto (follow node)**. The service probes the exit associated with the selected device/node, resolves it to a city-level Geo result, and uses that result as the WLOC target. It does not claim to know the phone’s GPS position.

选择 **Auto (follow node)**。服务会探测测试设备所绑定节点的真实出口，将其解析为城市级 Geo 结果，并作为 WLOC 目标。它不代表知道手机的 GPS 位置。

### Manual mode / 手动模式

Select manual mode and either:

- search a place name such as `London, UK`; or
- enter latitude and longitude explicitly; or
- apply a saved preset.

选择手动模式后，可以搜索 `London, UK` 等地点、直接输入经纬度，或应用已保存的预设。

Use city-level values only. Do not enter a precise home address or another person’s location.

只使用城市级位置，不要输入家庭精确地址或他人的位置。

## 7. 触发手机更新 / Trigger an iPhone refresh

After changing the node or target location:

修改节点或目标位置后：

1. Save the WLOC settings.
2. Toggle Wi‑Fi off/on, or briefly enable/disable Airplane Mode.
3. Reopen Maps or Weather and wait for a new network-location request.
4. If the app caches the old result, force-close it and retry once.
5. Keep all device VPNs off during the refresh.

Do not use a Shadowrocket/Loon WLOC module at the same time as the router interception. Two proxy paths can prevent the Apple handshake from completing.

不要同时运行 Shadowrocket/Loon 的 WLOC 模块和路由器拦截。双重代理可能导致 Apple 握手失败。

## 8. 监控与验收 / Monitoring and acceptance

Open **WLOC Monitor & Log** and confirm the state, target, source and update time.

打开 **WLOC Monitor & Log**，确认状态、目标位置、来源和更新时间。

![WLOC monitor and log](images/wificalling-wloc/05-wloc-monitor.png)

Expected indicators:

| Indicator | Meaning / 含义 |
|---|---|
| `intercepting` | The scoped WLOC path is enabled; it is not proof of emergency-location correctness. / 已启用受限 WLOC 路径，不代表紧急呼叫位置正确。 |
| `fresh` | Geo data is current and bound to the observed exit. / Geo 数据新鲜且绑定当前出口。 |
| `uncertain` | Providers disagree; the service should not invent coordinates. / provider 冲突，服务不应伪造坐标。 |
| `unavailable` | No safe Geo result; original response remains unchanged. / 没有安全 Geo 结果，原响应保持透传。 |
| `Target updated` | A target metadata event was written; raw WLOC bodies are not logged. / 目标元数据已更新，不记录原始 WLOC 内容。 |

![Current location and usage log](images/wificalling-wloc/05-wloc-monitor.png)

### Acceptance checklist / 验收清单

- [ ] The iPhone is the assigned test device and has a stable DHCP binding.
- [ ] Wi‑Fi Calling monitor is independently healthy.
- [ ] WLOC is limited to the two exact Apple hostnames and TCP 443.
- [ ] Ordinary HTTPS still shows the site’s normal certificate, not the WLOC CA.
- [ ] UDP 500/4500 behavior is unchanged.
- [ ] Switching node or manual preset updates the WLOC target after a refresh.
- [ ] Invalid Geo, provider outage, expired data and service failure leave the original path intact.
- [ ] Logs contain metadata only—never raw response bodies, keys or device identifiers.

## 9. FAQ / 常见问题

![Help and FAQ](images/wificalling-wloc/06-help-faq.png)

### WLOC shows `unavailable` / WLOC 显示 `unavailable`

Check node connectivity, the device-to-node binding, WAN/exit observation and Geo provider reachability. Do not replace the missing result with a fixed default coordinate.

检查节点连通性、设备到节点的绑定、WAN/出口探测和 Geo provider 连通性。不要用固定默认坐标替代缺失结果。

### Wi‑Fi Calling is not registered / Wi‑Fi Calling 未注册

Check the device policy, node health, DHCP binding and UDP 500/4500 path. Disable WLOC temporarily to isolate the two data planes. WLOC must never intercept the Wi‑Fi Calling tunnel.

检查设备策略、节点健康、DHCP 绑定以及 UDP 500/4500 路径。可以暂时关闭 WLOC 以隔离两个数据面。WLOC 绝不能拦截 Wi‑Fi Calling 隧道。

### Maps still shows the old location / 地图仍显示旧位置

Refresh the iPhone network location, clear the app’s stale session, confirm no device VPN is active, and inspect the monitor’s `observed_at`/`Target updated` entries. City-level IP Geo can also differ from a real physical location.

重新触发 iPhone 网络定位，清理应用旧会话，确认没有设备 VPN，并检查监控页的 `observed_at` / `Target updated`。城市级 IP Geo 也可能与真实物理位置不同。

### HTTPS fails after installing the CA / 安装 CA 后 HTTPS 失败

Verify full trust is enabled, the fingerprint matches, the hostname is one of the two approved WLOC names, and the iPhone date/time is correct. Ordinary HTTPS must not be routed through the WLOC CA.

确认已开启完全信任、指纹匹配、域名属于两个允许的 WLOC 域名，并检查手机时间。普通 HTTPS 不应经过 WLOC CA。

### How do I disable and roll back? / 如何关闭与回滚？

1. Turn off **Enable WLOC interception** and click **Save & Apply**.
2. Confirm the service returns to `disabled` or `degraded_passthrough`.
3. Remove only the WLOC redirect/table using the project’s service script.
4. Stop/disable `wloc-service`.
5. Keep or revoke the CA according to your lab policy; delete the private key only after confirming no test device still trusts it.

1. 关闭 **Enable WLOC interception** 并点击 **Save & Apply**。
2. 确认服务回到 `disabled` 或 `degraded_passthrough`。
3. 只删除项目自己的 WLOC redirect/table。
4. 停止并禁用 `wloc-service`。
5. 按实验室策略保留或撤销 CA；确认没有测试设备继续信任后再删除私钥。

## 10. 安全、隐私与合规 / Security, privacy and compliance

- This is a controlled lab feature for an authorized test device.
- The target is a network-location response, not GPS, base-station truth or a verified physical address.
- Never use a spoofed location for emergency services, dispatch, navigation safety or carrier compliance claims.
- Never commit CA private keys, node credentials, raw captures, device identifiers, precise real locations or provider tokens.
- WLOC logs are metadata only. Clear them when the test window ends.
- Keep the stable Wi‑Fi Calling Gateway 1.7 data plane isolated. Do not flush or rewrite its nftables table.
- When any safety prerequisite is uncertain, leave the redirect absent and pass through the original response.

本功能只适用于经授权的实验设备。目标是网络定位响应，不是 GPS、基站真实位置或经过验证的物理地址。禁止将伪造位置用于紧急服务、调度、导航安全或运营商合规证明。任何安全前置条件不确定时，应保持 redirect 缺失并透传原始响应。

## 11. 相关文档 / Related documentation

- [AX6S deployment guide](deployment/AX6S_DEPLOYMENT.md)
- [WLOC service API](api/WLOC_SERVICE_API.md)
- [UI integration design](ui/INTEGRATION_UI.md)
- [Threat model](security/threat-model.md)
- [Fail-open rules](security/fail-open.md)
- [Development and test plan](../DEVELOPMENT_TEST_PLAN.md)
