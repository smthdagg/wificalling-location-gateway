# Wi‑Fi Calling & WLOC 完整使用教程

适用于集成了 Wi‑Fi Calling Gateway 1.7 与路由器端 WLOC 服务的 OpenWrt / ImmortalWrt 网关。本教程从软件已经安装完成、可以进入 LuCI 管理页面开始。

[English tutorial](WIFICALLING_WLOC_TUTORIAL_EN.md)

## 一、使用前先了解

这个项目包含两个并列模块：

- **Wi‑Fi Calling Gateway**：让指定局域网设备通过指定的 sing-box 节点联网，并观察 ePDG/IPsec UDP 500、4500 会话。
- **WLOC**：由路由器为指定测试 iPhone 处理 Apple 网络定位请求，支持自动跟随节点出口位置或手动指定位置。

两者共享节点和设备策略，但功能边界不同。WLOC 不需要 Cloudflare Worker、WLOC Plus 后台、TOKEN、Shadowrocket 模块或手机端 HTTPS 解密。测试时必须关闭 Shadowrocket、Cloudflare WARP、Loon、WireGuard 等手机 VPN，避免绕过路由器。

> 本功能只应用于已获授权的专用测试设备。WLOC 不能替代 GPS、基站定位或运营商的紧急呼叫地址。不要用虚拟位置测试紧急呼叫。

## 二、准备条件

1. 路由器已安装并运行 Wi‑Fi Calling & WLOC 网关。
2. sing-box 可用，并准备好与 SIM 卡归属国家或地区一致的出口节点。
3. 测试 iPhone 已连接此路由器 Wi‑Fi。
4. iPhone 使用稳定的局域网 IPv4 地址；插件会通过 DHCP 静态租约维护设备策略与地址的绑定。
5. 手机端所有 VPN、代理和全局隧道均已关闭。

节点建议优先使用 **AnyTLS、VLESS、VMess 或 Trojan** 等 TCP 系协议。Hysteria2、TUIC 等 UDP/QUIC 协议可能可以看到 Wi‑Fi Calling 图标，但在公网抖动时通话容易中断。WireGuard 可用，但仍需实测稳定性。

## 三、配置 Wi‑Fi Calling Gateway

进入 LuCI：**服务 → WifiCalling&Wloc Gateway → Wi‑Fi 通话设置**。

![Wi‑Fi Calling 设置页面](images/wificalling-wloc/01-wifi-calling-settings.png)

### 3.1 基本设置

1. 勾选“启用”。
2. 日志级别通常保持 `Warning`。
3. 如需记录握手和持续加密通讯，开启“活动日志”。
4. 持续活动日志间隔可保持 60 秒，每台设备最大记录数可保持 20 条。

### 3.2 添加代理节点

可以点击“添加代理节点”手动填写，也可以在“导入代理节点”中粘贴一条 AnyTLS、Hysteria2/Hy2、TUIC、VLESS、VMess、Trojan 或 WireGuard 分享链接。

导入只在当前浏览器本地解析。导入后仍要核对服务器、端口、密码或 UUID、SNI、TLS、Reality、WebSocket 等字段，然后点击一次“保存并应用”。节点必须先保存，才能在设备策略中选择。

### 3.3 添加 iPhone 设备策略

在“设备策略”中点击“添加局域网设备”，填写：

1. 容易识别的设备名称。
2. **独立通道**路由模式。
3. 前一步保存的出口节点。
4. iPhone 当前使用的局域网 IPv4 地址。

再次点击“保存并应用”。检查“DHCP 绑定”一栏应显示“已绑定”。如果显示待绑定、MAC 已变化或设备离线，让 iPhone 关闭再打开 Wi‑Fi，重新获取地址后再检查。

![代理节点与设备策略](images/wificalling-wloc/02-device-policies.png)

## 四、在 iPhone 开启 Wi‑Fi Calling

1. 打开 **设置 → 蜂窝网络**。
2. 选择对应 SIM 或号码。
3. 进入 **Wi‑Fi 通话**。
4. 开启“在此 iPhone 上进行 Wi‑Fi 通话”。

不同运营商和 iOS 版本的名称可能略有不同。如果完全没有这个选项，应先向运营商确认该 SIM、套餐和地区是否支持 Wi‑Fi Calling。

## 五、在 iPhone 安装 WLOC 根证书

WLOC 的证书由路由器生成，不使用 Shadowrocket 证书。

### 5.1 下载并安装描述文件

1. 在测试 iPhone 的 Safari 中打开 `http://192.168.31.1/wloc-ca.mobileconfig`。如果路由器 LAN 地址不是 `192.168.31.1`，请换成实际地址。
2. Safari 提示已下载描述文件后，打开 iPhone“设置”。
3. 点击顶部的“已下载描述文件”；如果没有显示，进入 **设置 → 通用 → VPN 与设备管理**。
4. 找到 WLOC 描述文件，按提示完成安装。

下图复用了 iOS Location Spoofer Plus 教程中的 iOS 系统页面。菜单路径相同，但本项目显示的描述文件名称应为 **wloc-service**，不是 Shadowrocket。

![iOS VPN 与设备管理](images/iphone/01-ios-vpn-device-management.jpg)

### 5.2 开启根证书完全信任

1. 进入 **设置 → 通用 → 关于本机 → 证书信任设置**。
2. 找到 **wloc-service root CA**。
3. 开启右侧的“完全信任”，并确认系统提示。

![iOS 证书信任设置](images/iphone/02-ios-certificate-trust.jpg)

证书只应安装在专用测试 iPhone。测试结束后，应删除描述文件并关闭对应根证书信任。

## 六、配置 WLOC

进入 LuCI：**服务 → WifiCalling&Wloc Gateway → WLOC 设置**。

![WLOC 设置页面](images/wificalling-wloc/04-wloc-settings.png)

### 6.1 选择跟随设备

在“跟随设备”中选择刚才配置的测试 iPhone。自动模式会读取该设备绑定的节点，以节点出口 IP 的地理位置作为 WLOC 目标。

### 6.2 自动模式

1. 将“定位模式”设为“自动（跟随节点）”。
2. 确认测试设备已绑定正确节点。
3. 开启“启用 WLOC 拦截”。
4. 点击“保存并应用”。

更换设备绑定节点后，WLOC 会在下次刷新时更新目标位置。

### 6.3 手动模式

需要固定位置时：

1. 在“手动搜索”中输入地点，例如 `London, UK`，点击“搜索”。
2. 确认搜索返回的城市、纬度和经度。
3. 点击“应用坐标”。只搜索而不点击“应用坐标”不会切换位置。
4. 也可以直接输入纬度和经度，或从“已保存的位置”中点击“应用”。
5. 保存后，定位模式会切换为手动；手动坐标保存在路由器本地配置中。

## 七、让新位置在 iPhone 生效

1. 确认 Shadowrocket、WARP 和其他 VPN 的总开关均已关闭。
2. 打开 **设置 → 隐私与安全性 → 定位服务**。
3. 关闭定位服务，等待约 5 至 10 秒，再重新开启。
4. 强制退出地图、天气等应用后重新打开。
5. 如果仍未刷新，可切换一次飞行模式，或关闭再打开 Wi‑Fi，然后等待片刻。

Apple 定位服务可能存在缓存。自动模式下，网页看到的公网 IP、WLOC 目标位置和设备绑定节点应指向同一国家或地区。

## 八、检查 WLOC 状态

进入 **WLOC 监控与日志**。

![WLOC 当前位置与日志](images/wificalling-wloc/05-wloc-monitor.png)

重点检查：

- `Service phase` 为 `intercepting`。
- `Follow device` 是当前测试 iPhone。
- `Location mode` 与所选自动或手动模式一致。
- 国家、城市、时区和 GPS 坐标符合目标。
- `Geo state` 为 `fresh`。
- WLOC 使用日志出现“定位目标更新”。

“定位目标更新”只证明路由器已经更新 WLOC 目标，不代表手机 GPS、基站位置或运营商紧急地址已经改变。

## 九、检查 Wi‑Fi Calling

进入 **Wi‑Fi 通话监控与日志**。

![Wi‑Fi Calling 监控与活动日志](images/wificalling-wloc/03-wifi-calling-monitor.png)

状态通常按以下顺序变化：

1. 观察到 UDP 500：正在协商。
2. 观察到 UDP 4500：已进入 NAT-T。
3. UDP 4500 为双向 `ASSURED`：页面显示“已注册”。
4. 注册后出现持续双向加密流量：日志可能显示“通话进行中（根据持续加密流量推断）”。

“已注册”是路由器侧网络证据，不是运营商激活确认。最终必须使用普通号码实际呼出和呼入验证。路由器无法看到加密隧道内的号码、短信、语音内容或呼叫方向。

也可以打开飞行模式，再单独开启 Wi‑Fi，观察状态栏是否出现运营商的 Wi‑Fi Calling 标识。下图是 Wi‑Fi Calling Gateway 项目的 iPhone 实机记录：

<img src="images/iphone/03-iphone-ee-wificall.jpg" alt="iPhone 显示 EE WiFiCall" width="420">

## 十、常见问题

### Wi‑Fi Calling 一直“未检测到”

检查节点是否可达、设备策略是否为独立通道、iPhone IP 是否与策略一致、DHCP 是否已绑定，以及 iPhone 的 Wi‑Fi Calling 开关是否开启。

### 显示“已注册”，但无法通话

“已注册”只代表观察到 ASSURED UDP 4500。优先改用稳定的 TCP 系节点，确认节点国家与 SIM 归属地匹配，并检查运营商是否真正为号码开通服务。

### WLOC 位置没有变化

确认 wloc-service 根证书已经安装并完全信任、WLOC 拦截已开启、跟随设备正确、手机 VPN 已关闭。然后重新开关定位服务并重启地图或天气应用。

### WLOC 自动位置与节点不一致

确认测试设备绑定的是预期节点，手机没有使用其他 VPN，Passwall 等全局代理没有抢走该设备流量，并等待 Geo 状态刷新。

### 如何立即恢复正常定位

在 WLOC 设置中关闭“启用 WLOC 拦截”，保存并应用；Apple WLOC 流量应恢复原始响应。需要彻底退出测试时，再删除 iPhone 上的 WLOC 描述文件和根证书信任。

![内置使用帮助](images/wificalling-wloc/06-help-faq.png)

## 十一、安全说明

- 不要公开代理节点链接、密码、UUID、私钥或证书私钥。
- 不要把真实设备标识、原始网络流量或精确个人位置提交到 GitHub。
- 只对获授权的测试 iPhone 使用根证书。
- WLOC 只应处理指定设备的 Apple WLOC 请求，不能用于普通 HTTPS 网站。
- 遵守当地法律、Apple 条款和运营商条款，并始终使用真实位置处理紧急服务。

## 资料来源

- [Wi‑Fi Calling Gateway 中文 README](https://github.com/smthdagg/luci-app-wificalling-gateway/blob/main/README.md)
- [iOS Location Spoofer Plus 完整安装使用教程](https://github.com/smthdagg/ios-location-spoofer-plus/blob/main/%E5%AE%8C%E6%95%B4%E7%9A%84%E5%AE%89%E8%A3%85%E4%BD%BF%E7%94%A8%E6%95%99%E7%A8%8B.md)
- 本项目当前 LuCI 页面、内置 FAQ 和 AX6S 实机验证记录
