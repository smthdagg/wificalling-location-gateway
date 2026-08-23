# Wi-Fi Calling + WLOC 开发测试计划

> 当前强制基线：本仓库只允许基于最后发布且已验证的 1.2.x 稳定整合包继续升级。下文保留的早期 1.7 分阶段记录仅用于追溯已经完成的设计来源，不是可用的构建输入、依赖或回退目标。多设备/2.0 Beta 已迁至独立仓库，不属于本项目的代码、文档、测试或发布范围；任何实现不得从该项目回迁。

## 1. 权威依据

本计划以 Codex 任务 `019feec1-a2dc-7a70-bdba-3c2fc0176b14` 为唯一会话依据。该任务标题为“部署 server.js 到 Cloudflare (2)”，已按游标完整读取 12 页、117 个回合。

会话最终形成的方案不是继续部署地图或 Cloudflare，而是：

> Wi-Fi Calling Gateway 检测每个代理节点的真实出口 IP，将 IP 解析为国家、城市、经纬度和时区，再由路由器侧 WLOC 引擎只拦截目标 iPhone 的 Apple WLOC 请求，使设备网络定位自动跟随所绑定的代理出口。

明确决策：

- 不需要地图；
- 不需要 Cloudflare Worker/KV/TOKEN；
- 不需要手机端 Shadowrocket WLOC 模块；
- iPhone 只需安装并信任一次网关 CA；
- 第一阶段建立独立实验项目 `wificalling-location-gateway`；
- PoC 成熟后再拆成可选的 `wloc-mitm` 引擎，与稳定的 Wi-Fi Calling Gateway 集成；
- `sing-box` 必须保留并复用，不能卸载；
- 第一轮在 Redmi AX6S 上以 `/tmp` 临时运行的 ARM64 PoC 验证。

## 2. 当前基线

### 2.1 Gateway 1.7.0

仓库：`/Users/henry/Documents/Codex/2026-08-05/tiao`

- 当前提交：`b7cbe60`，与 `origin/main` 一致；
- 45 项测试通过；
- Shell `sh -n` 全部通过；
- LuCI JavaScript `node --check` 全部通过；
- 现有能力：节点管理、按设备分流、sing-box、nftables TPROXY、DHCP/MAC 自愈、UDP 500/4500 监控、活动日志、IPK/APK。

### 2.2 参考路由器

会话中已只读确认：

- Redmi AX6S；
- MediaTek MT7622，ARMv8 双核约 812MHz；
- ImmortalWrt 24.10.6，Linux 6.6.133；
- 总内存约 236.6MiB，当时可用约 49.7MiB；
- 持久存储约 85.6MiB，当时剩余约 21–26MiB；
- `/tmp` 剩余约 84.8MiB；
- 已安装 sing-box 1.13.16；
- Gateway 本体约 96KB，删除它只能释放约 0.1MiB；
- sing-box 解压后二进制约 43.4MB，且由现有代理能力复用，不能删除。

### 2.3 WLOC 参考实现

参考仓库：`mekos2772/ios-location-spoofer`。

- 已验证其包含 Apple WLOC response patch、location picker 和多代理客户端脚本；
- 许可证为 AGPL-3.0；
- Gateway 主项目为 MIT。

因此：不能直接把其源码复制进 MIT Gateway 包。实验项目必须独立许可；如果复用或派生 AGPL 代码，实验项目及网络服务修改需要按 AGPL 履行源代码义务。

## 3. 目标与非目标

### 3.1 PoC 目标

1. 指定一台测试 iPhone。
2. 复用 Gateway 中该设备绑定的 sing-box 节点。
3. 通过该节点检测真实出口 IP，而不是路由器 WAN IP。
4. 将出口 IP 解析为国家、城市、城市中心坐标和时区。
5. 只拦截该设备访问：
   - `gs-loc.apple.com`
   - `gs-loc-cn.apple.com`
6. 支持 Apple WLOC 所需 TLS、HTTP/2、二进制/protobuf 响应解析和修改。
7. 返回合理的城市级低精度位置。
8. 不运行 Shadowrocket 时，iPhone 网络定位随节点出口变化。
9. 不影响普通 HTTPS、UDP 500/4500、Wi-Fi Calling 注册和真实通话。
10. PoC 失败后重启或执行停止脚本即可完整恢复。

### 3.2 非目标

- 不做地图和搜索选点；
- 不部署 Cloudflare Worker、KV 或 token；
- 不拦截所有 HTTPS；
- 不内置城市级 GeoIP 数据库；
- 不支持多设备并发；
- 不首发多架构；
- 不首期修改 Gateway 1.7 稳定代码；
- 不承诺运营商激活、真实 GPS 或紧急呼叫位置准确。

## 4. 推荐架构

```mermaid
flowchart LR
    A["设备策略：iPhone → sing-box 节点"] --> B["Node Exit Probe"]
    B --> C["经指定节点查询真实出口 IP"]
    C --> D["Geo Resolver"]
    D --> E["国家/城市/坐标/时区缓存"]
    A --> F["nftables：仅测试设备 + WLOC 域名"]
    F --> G["wloc-mitm"]
    E --> G
    G --> H["Apple WLOC 上游"]
    G --> I["修改后的 WLOC 响应"]
    I --> J["iPhone 网络定位"]
    A --> K["现有 Gateway 数据面"]
    K --> L["ePDG UDP 500/4500"]
```

### 4.1 Node Exit Probe

职责：

- 为每个使用中的节点启动隔离的临时探测；
- 确保请求经过指定 sing-box outbound；
- 查询出口 IP；
- 记录 `node_id / exit_ip / checked_at / probe_state`；
- 不改写正在运行的生产 sing-box 配置；
- 不记录节点密码、UUID、私钥或完整分享链接。

必须防止两类错误：

- 查询到路由器 WAN IP；
- 只用 ICMP/tcping 便把节点判定为真实代理可用。

### 4.2 Geo Resolver

PoC 使用在线 IP 地理位置服务并缓存：

```json
{
  "node_id": "uk_anytls",
  "exit_ip": "203.0.113.1",
  "country_code": "GB",
  "city": "London",
  "latitude": 51.5074,
  "longitude": -0.1278,
  "timezone": "Europe/London",
  "checked_at": 0,
  "expires_at": 0
}
```

要求：

- 出口 IP 未改变时不重复查询；
- 至少支持主/备 provider；
- provider 返回冲突时标记 `geo_uncertain`，不静默选错；
- API 不可用时使用未过期缓存；
- 没有可靠数据时 WLOC 引擎透传原响应；
- 不回落到 Cupertino 或其他固定默认坐标；
- 对经纬度、国家码、时区做严格校验。

### 4.3 wloc-mitm

职责：

- 终止 iPhone 到 Apple WLOC 域名的 TLS；
- 使用路由器本地 CA 动态签发仅限目标 hostname 的证书；
- 支持 TLS 1.2/1.3、ALPN 和 HTTP/2；
- 转发上游请求；
- 解析原始 WLOC 二进制/protobuf 响应；
- 只修改与定位相关的坐标和精度字段；
- 保留未知字段和响应结构；
- 将上游错误、未知协议或解析失败安全透传。

建议响应使用城市级、非导航级精度：

```text
latitude/longitude：出口城市中心或可靠 IP 库坐标
horizontalAccuracy：建议 3–10km，最终以真机行为验证
verticalAccuracy：保守低精度
altitude：缺数据时不伪造高精度值
```

### 4.4 流量隔离

nftables 必须使用独立的 `wificalling_location` table/chain，禁止复用或改写现有 `wificalling_gateway` 数据面规则。现有规则会把策略设备的全部 TCP/UDP 送入 sing-box，不具备 WLOC 域名级 MITM 隔离能力。

nftables 规则必须同时匹配：

- 测试设备源 IP；
- Apple WLOC 目标集合；
- TCP 443。

不得拦截：

- 其他 LAN 设备；
- 测试 iPhone 的普通 HTTPS；
- UDP 500/4500；
- Gateway 的 sing-box 管理和健康检查流量；
- 路由器管理页面。

WLOC 域名解析得到的 IP 应通过 dnsmasq nftset/ipset 动态维护，并处理地址变化和 IPv6。

IPv6 是 PoC 的硬门禁：现有 Gateway 只有 `clients4` 和 IPv4 私网校验。如果 iPhone 通过 AAAA 访问 Apple WLOC，可能绕过 MITM 或形成 IPv4/IPv6 分裂状态。PoC 必须二选一并明确测试：

1. 完整实现 `clients6`、WLOC destination v6 nft set 和 IPv6 redirect；或
2. 在测试设备范围内显式抑制 WLOC AAAA，并证明普通 IPv6 流量策略不被意外改变。

未完成其中之一，不允许连接真机。

### 4.5 CA 管理

- 首次启动在路由器本地生成专用 CA；
- CA 私钥权限 0600，只允许 root 读取；
- LuCI 仅提供 CA 公钥证书下载、SHA-256 指纹和安装说明；
- 不上传 CA 私钥，不写入日志或支持包；
- 提供重新生成、撤销信任和删除证书步骤；
- 引擎禁用后不得继续保留透明转发规则；
- CA 与 Gateway 节点密钥分开存储和备份。

CA 生命周期必须自动化验收：

- 私钥始终 0600；
- LuCI、日志、支持包和普通配置备份不包含私钥；
- leaf certificate SAN 只能是两个精确 Apple hostname；
- CA 重新生成后必须清空旧 leaf cache；
- stop 操作先撤销 redirect，再停止引擎；
- 卸载时询问是否删除 CA，避免无提示破坏用户已信任的证书链。

### 4.6 精确定义 fail-open

| 故障 | 必须行为 |
|---|---|
| Geo provider 不可用且无有效缓存 | 转发 Apple 原始 WLOC 响应，不修改位置 |
| WLOC parser/patch 失败 | 转发原始响应，不返回默认坐标 |
| Apple 上游证书验证失败 | 不生成位置；返回受控失败并记录最小错误信息 |
| MITM 进程死亡或健康检查失败 | watchdog 立即移除/禁用 WLOC redirect，不能长期黑洞 |
| 节点出口探测失败 | 保留最后有效且未过期缓存；否则透传 |
| 响应超过大小/解压上限 | 不解析、不缓存、不记录正文；按策略透传或受控失败 |

## 5. 独立项目结构

建议新建独立仓库：

```text
wificalling-location-gateway/
  cmd/wloc-mitm/
  internal/
    ca/
    exitprobe/
    georesolver/
    proxy/
    wloc/
  fixtures/
    wloc/
  openwrt/
    root/etc/init.d/wificalling-location
    root/etc/config/wificalling-location
    root/usr/libexec/wificalling-location/
    root/usr/share/luci/
  scripts/
    build-openwrt.sh
    deploy-poc.sh
    rollback-poc.sh
  tests/
  SECURITY.md
  LICENSE
  README.md
```

实验项目不得直接在 Gateway 1.7 仓库内开发，以免引入 TLS/CA/二进制依赖和许可证污染。

## 6. 技术选型门禁

### 6.1 实现语言

会话建议第一版使用 Go，因为 TLS、HTTP/2、protobuf 和交叉编译支持成熟。资源门禁：

- ARM64 压缩包目标 ≤5–8MB；
- 安装后目标 ≤8–18MB；
- 空闲 RSS ≤20MB；
- 单次请求峰值 RSS ≤35MB；
- AX6S 实测峰值必须 ≤25MB 才进入长期驻留评估。

若 stripped Go 二进制持续超出 AX6S 门禁，再评估 Rust；不为省几 MB 直接选择手写 C TLS/HTTP2。

### 6.2 GeoIP

PoC：在线 provider + 本地缓存。

暂不内置：

- 国家库约 5–15MB 安装占用；
- 城市库约 50–120MB 安装占用；
- 当前 AX6S 剩余闪存不适合城市库。

### 6.3 包拆分

成熟后建议：

```text
wificalling-location-engine
  架构相关 wloc-mitm 二进制

luci-app-wificalling-location
  架构无关 LuCI、UCI、init、ACL
```

Gateway 1.7/后续版本只声明可选集成，不把引擎设为强制依赖。

## 7. 资源预算

### PoC 硬门禁

| 项目 | 上限 |
|---|---:|
| `/tmp` 二进制与临时文件 | 8MB |
| 运行内存目标 | 15–25MB |
| 持久 UCI/CA/缓存 | 200KB |
| 持久日志 | 1MB |
| 并发测试设备 | 1 |
| WLOC 拦截域名 | 2 |

运行时还必须限制：

- 总连接数；
- 单设备连接数；
- HTTP/2 concurrent streams；
- 压缩前/后 body 大小及解压倍率；
- 请求、握手、上游连接和空闲超时；
- procd respawn 频率；
- 日志轮转总量 ≤1MB。

### 正式包预估

| 项目 | 下载包 | 安装后 |
|---|---:|---:|
| LuCI/脚本/配置 | 20–100KB | 100–300KB |
| CA/缓存 | 可忽略 | 50–200KB |
| wloc-mitm | 3–8MB | 8–18MB |
| 合计 | 约 4–8MB | 约 10–20MB |

在 AX6S 上，只有实际安装后仍保留至少 10MiB 系统余量，才允许从 `/tmp` PoC 转为持久安装。

## 8. 开发阶段

### Phase 0：证据与协议冻结（3–5 天）

- 获取合法授权的 WLOC 请求/响应测试样本；
- 确认 protobuf 字段、重复字段、未知字段保留规则；
- 确认 iOS 版本、Apple hostname、TLS/ALPN/HTTP2 行为；
- 确认参考 WLOC 实现许可证和可复用范围；
- 完成威胁模型和 ADR。

退出条件：没有真实设备流量就不开始实现 response patch。

额外硬门禁：在 Phase 0 必须书面选择许可证路线：

- 复用/派生参考实现 → 独立实验项目采用 AGPL 并提供对应源码；或
- 未来希望 MIT 兼容 → 使用 clean-room fixtures/协议笔记，开发者不得移植参考实现源码。

### Phase 1：Node Exit Probe（2–4 天，TDD）

- 从 Gateway 导出的最小节点配置建立临时 outbound；
- 通过指定节点查询出口 IP；
- 超时、错误凭据、DNS、节点黑洞测试；
- 进程/端口/临时文件清理；
- 输出脱敏 JSON。

退出条件：能稳定区分 WAN IP、可达节点和真实代理出口。

### Phase 2：Geo Resolver（2–3 天，TDD）

- provider adapter；
- schema 校验；
- 主备冲突；
- 缓存/过期；
- 无数据 fail-open；
- 无默认假坐标。

退出条件：节点出口改变时坐标更新；API 故障时行为可预测。

### Phase 3：离线 WLOC Patch 核心（5–8 天，TDD）

- fixture parser；
- 字段定位和坐标替换；
- 未知字段保留；
- malformed/truncated/oversized input；
- 多响应/边界值；
- fuzzing。

退出条件：所有 fixture 可 round-trip；非目标字段字节级保持或语义保持。

### Phase 4：TLS/HTTP2 MITM（7–10 天，TDD）

- CA、叶证书、SAN；
- TLS 1.2/1.3；
- ALPN h2/http1.1；
- HTTP/2 streaming/body limits；
- 上游证书验证；
- 超时与连接复用；
- 解析失败透传；
- 普通域名拒绝代理。
- 连接/stream/body/解压/内存硬上限；
- 引擎死亡时的 redirect 自动撤销。

退出条件：只对测试域名工作；测试 CA 未安装时失败方式明确；普通 HTTPS 无影响。

### Phase 5：OpenWrt `/tmp` PoC（4–6 天）

- ARM64 交叉编译；
- 临时 init/stop/rollback 脚本；
- dnsmasq nftset + nftables 精确重定向；
- 独立 `wificalling_location` table/chain，不修改现有 Gateway table；
- IPv6 完整路径或 WLOC AAAA 精确抑制；
- 单设备绑定；
- 资源与日志上限；
- 路由器重启自动清理。

退出条件：停止脚本和重启都能恢复原始网络；不需要卸载 sing-box。

### Phase 6：iPhone 真机验证（5–8 天）

按顺序测试：

1. 保留 Gateway 1.7 配置备份；
2. 固定测试 iPhone IP；
3. 安装并核对网关 CA 指纹；
4. 不运行 Shadowrocket；
5. 选择 UK 节点，确认出口 IP/城市；
6. 触发 Apple 网络定位，确认定位到 UK 城市；
7. 切换 US/HK 节点，确认定位跟随；
8. 检查 Safari 普通 HTTPS 证书未被网关签发；
9. 检查 UDP 500/4500 和 ASSURED；
10. 真实呼入/呼出测试；
11. 引擎停止、CA 撤销、网络恢复；
12. 重复至少 3 次。

退出条件：成功可重复、失败可恢复、没有普通 HTTPS 扩大拦截。

### Phase 7：LuCI 与持久包（4–6 天）

仅在 PoC 通过后开发：

- 开关、单设备选择、自动跟随出口；
- 出口 IP、国家、城市、坐标、时区、缓存时间；
- CA 下载、指纹、安装/撤销说明；
- 引擎状态和最近错误；
- 不显示地图；
- 不显示虚假的“Wi-Fi Calling 已激活”；
- IPK/APK、升级和卸载脚本。

### Phase 8：Gateway 可选集成（3–5 天）

- Gateway 检测可选 engine 是否安装；
- 通过稳定 JSON/ubus 契约提供 node/device 映射；
- feature flag 默认关闭；
- 引擎故障不得阻止 Gateway 启动；
- Gateway 1.7 旧配置零变化；
- 独立包可单独卸载。

总工作量：约 **35–55 个开发日**，适合分为 PoC、真机验证、产品化三个里程碑，不应按普通 LuCI 功能估算。

## 9. 测试矩阵

### 9.1 WLOC 离线单元测试

- 合法响应解析/重编码；
- 经纬度正负、边界和精度；
- 多 AP/多 cell 记录；
- 未知字段保留；
- 字段顺序变化；
- malformed/truncated/oversized body；
- gzip/压缩响应和解压倍率炸弹；
- 非 WLOC protobuf 拒绝；
- fuzz 测试不 panic、不越界、不无限分配。

### 9.2 TLS/HTTP2

- CA/叶证书签发与过期；
- SAN 精确匹配；
- TLS 1.2/1.3；
- ALPN h2；
- 多 stream；
- h2 stream 数量、连接数、body 与解压上限；
- upstream cert invalid；
- body size/timeouts；
- 未信任 CA；
- 非 allowlist hostname 不签发、不代理；
- CA 私钥权限和日志泄露测试。

### 9.3 Exit/Geo

- 经指定节点得到出口 IP；
- 不得得到 WAN IP；
- 节点 host reachable 但代理失败；
- provider 超时/限流/坏 JSON/冲突；
- 缓存命中/过期；
- exit IP 变化；
- IPv4/IPv6；
- 无可靠位置时透传。

### 9.4 OpenWrt 集成

- 只匹配指定设备；
- 只匹配两个 WLOC 域名；
- 独立 nft table，不修改 `wificalling_gateway`；
- WLOC A/AAAA 地址变化与 IPv6 路径；
- 普通 HTTPS 不进 MITM；
- UDP 500/4500 不进 MITM；
- stop/restart/crash 后 nftables 清理；
- dnsmasq reload；
- Gateway/sing-box 并存；
- 资源上限和日志轮转；
- reboot 恢复。
- MITM 进程 kill -9 后 redirect 自动撤销；
- CA 轮换、leaf cache 清理、卸载保留/删除选择。

### 9.5 真机负向场景

| 场景 | 预期 |
|---|---|
| 未安装 CA | WLOC MITM 不成功，但普通网络正常 |
| CA 已撤销 | 定位恢复真实/Apple 原始响应 |
| Geo API 离线且无缓存 | 原始响应透传，不返回默认假位置 |
| 节点黑洞 | 不更新位置，Gateway 状态显示失败 |
| 节点国家切换 | 缓存更新后定位跟随 |
| iOS 定位缓存 | 显示引擎已修改但提示设备缓存，按版本执行重启 |
| GPS 信号很强 | 网络位置可能不主导，不能误判引擎失败 |
| 其他 LAN 设备 | 证书和流量完全不受影响 |
| 引擎崩溃 | nftables watchdog/stop 清理，网络恢复 |
| 路由器重启 | PoC 自动消失，正式版按配置恢复 |

## 10. 安全与合规门禁

### 必须满足

- 用户明确授权测试自己的 iPhone 和局域网；
- UI 明确说明存在 TLS MITM；
- 只拦截两个 Apple WLOC 域名；
- CA 私钥只在路由器本地；
- 不记录完整 WLOC 请求、Wi-Fi/BSSID/cell 原始数据；
- 不记录节点密钥；
- 调试包自动脱敏；
- 引擎默认关闭；
- 提供一键停止、规则清理和 CA 撤销步骤；
- 不宣传为紧急呼叫位置保证；
- 不把网络证据描述为运营商激活结论。

日志 schema 只允许：事件类型、精确 allowlist hostname、成功/失败结果、请求/响应字节数、粗粒度国家/城市和经过散列的设备标识。支持包必须有自动脱敏测试。

### 许可证

- Gateway：MIT；
- `mekos2772/ios-location-spoofer`：AGPL-3.0；
- 新项目若复用其代码，必须保留 AGPL 边界和源代码提供机制；
- 若采用独立实现，也需记录 clean-room 证据和所用协议资料来源；
- PoC 通过前不得复制代码进入 Gateway 仓库。

## 11. 发布/回滚策略

### PoC

- 二进制和规则放 `/tmp`；
- 不创建正式 package；
- 不删除 sing-box；
- 不覆盖 Gateway 配置；
- 测试前备份 UCI/nftables/证书状态；
- `rollback-poc.sh` 删除进程、规则、nft set 和临时证书；
- 重启作为最终恢复手段。

### 产品化

- 独立 engine/LuCI 包；
- feature flag 默认关闭；
- 卸载前先停止和清理规则；
- 卸载时 CA 私钥需用户确认后删除；
- Gateway 对 engine 只做可选调用；
- engine 不存在或失败时 Gateway 继续正常工作。

## 12. 第一迭代任务清单

第一迭代只执行 Phase 0–2：

1. 建立独立仓库和许可证边界；
2. 固化真实 WLOC fixtures；
3. 输出协议与威胁模型 ADR；
4. 开发指定节点出口 IP 探测；
5. 开发在线 Geo resolver + 缓存；
6. 在 Mac/Linux 上完成全部单元测试；
7. 暂不连接 iPhone、暂不生成 CA、暂不修改路由器。

完成评审后才进入 WLOC patch 和 TLS MITM，避免在协议未冻结前直接操作真机网络。

安全门禁：Phase 0–2 结束时如果独立 nft/IPv6 设计、CA 生命周期测试、fail-open 状态机或许可证路线仍未冻结，项目继续停留在离线阶段，不进入 MITM 开发。
