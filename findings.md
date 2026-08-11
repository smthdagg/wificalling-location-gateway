# 调研发现

## 会话存档

- 用户提供的 `.zcode-session` 路径不存在；同一会话 ID 的实际记录位于 `/Users/henry/.zcode/cli/rollout/model-io-sess_666e1cc5-3fc4-4575-8eb8-3d4a068823e6.jsonl`。
- 会话记录显示 Wi-Fi Calling Gateway 已发布 1.7.0，公开仓库为 `smthdagg/luci-app-wificalling-gateway`，本地开发目录为 `/Users/henry/Documents/Codex/2026-08-05/tiao`。
- 1.7.0 已完成 45 项测试、IPK/APK 构建、路由器部署和 GitHub Release。
- 当前能力包括 LuCI 配置、sing-box 多协议出口、设备策略、DHCP 静态租约自动管理与绑定状态。
- 已知经验：网关出口优先 TCP 协议；Hysteria2/TUIC 的 ICMP 存活不等价于可用互联网；iOS 私有 MAC 变化需要 DHCP 自动修复；订阅节点标签可能含 `|`。
- 进一步从会话中的 assistant 历史内容还原了现成方案：定位杭州的直接原因曾是 iPhone 私有 MAC 变化使设备未命中策略；1.7.0 已用 `dhcp-sync.sh` 与 LuCI 绑定状态解决。该结论只覆盖出口 IP 路径，不等价于 Apple WLOC 坐标覆盖。
- 已完整读取 Codex 会话 `019fcf4e-d353-7d83-ab70-aef84c8d0cd0` 的 11 页、105 个回合。
- 该会话的真机成功基线是 AnyTLS 英国节点 + WLoc 英国定位 + 飞行模式/Wi-Fi，并成功拨通 888。
- 该会话已经明确产品边界：独立路由器插件负责网络，WLoc 由手机端完成。
- 会话中的 UK/CTE 场景与后续 HK 场景可能属于不同设备/SIM 或需求变化，因此新设计必须按设备配置国家，不硬编码 UK/HK。
- 上述 `019fcf4e...` 不是用户最终指定的方案任务，已降级为历史背景。
- 正确任务 `019feec1-a2dc-7a70-bdba-3c2fc0176b14` 已完整读取 12 页、117 回合。
- 正确任务的最终方案：取消地图和 Cloudflare，由路由器根据设备绑定节点的真实出口 IP 解析城市位置，并通过本地 WLOC MITM 返回给 iPhone。
- 正确任务决定先建立独立项目 `wificalling-location-gateway`，成熟后作为 Gateway 可选组件。
- 当前 AX6S 适合 `/tmp` ARM64 单设备轻量 PoC，不适合 Python/mitmproxy、城市 GeoIP 库或全量 HTTPS MITM。
- sing-box 必须保留；删除 Gateway 本体仅释放约 0.1MiB，不能解决存储问题。

## wloc 工作区

- 当前用户指定的集成工作区 `/Users/henry/Documents/ChatGPT/WifiCaling+wloc` 初始为空 Git 仓库，无产品源代码。
- 用户进一步指定现有项目交接文档 `/Users/henry/Documents/Codex/2026-08-05/tiao/DEVELOPER.md`，后续以该目录作为 Wi-Fi Calling 现状基线。
- `DEVELOPER.md` 明确当前链路：UCI → `normalized.conf`/clients/nodes → sing-box 配置 → nftables TPROXY；`dhcp-sync.sh` 管理设备绑定；monitor 基于 UDP 500/4500 conntrack 生成状态与事件。
- `README_EN.md` 明确产品边界：网关只提供对应国家出口 IP 和 Wi-Fi Calling 可观测证据，不控制 GPS/基站/wloc，也不修改运营商账户、IMS 或紧急呼叫地址。
- 文档把 “设备定位与 SIM 归属国一致” 作为 Wi-Fi Calling 前置条件，并引用 `smthdagg/ios-location-spoofer` + Shadowrocket 作为独立解决方案。
- 本机发现可能相关目录 `/Users/henry/Documents/Zcode/ios-location-picker-cloudflare-webui`，尚待判断它是 wloc 控制面、地图选点器还是独立 Web UI。
- 已确认该目录是 Cloudflare Worker 地图选点与 `loc.json` 坐标分发原型，供 Shadowrocket/Loon `configUrl` 拉取；不是 OpenWrt 运行模块。
- 公共 `Yu9191/wloc` 当前通过 `/wloc-settings/save` 写入设备端持久化存储，再在 `/clls/wloc` 响应中 patch protobuf；支持 Shadowrocket 等客户端，许可证为 AGPL-3.0。

## 集成需求与约束

- 现有网关支持 OpenWrt/ImmortalWrt/iStoreOS，LuCI JS、Shell、sing-box、firewall4/nftables，目标包为 IPK/APK。
- 首版集成不能破坏当前单 sing-box、按设备透明代理、DHCP 自动修复和 LuCI 24.10 的特殊 Save/Apply 逻辑。
- 定位功能涉及紧急呼叫与运营商合规边界，必须清楚区分技术状态与业务承诺，避免把 `likely_registered` 或位置覆盖描述为通话/紧急服务保证。

## 风险与未决问题

- 尚需确定 `wloc` 的具体实现、目录、输入输出与运行平台；不能仅凭名称假设。
- 原始会话记录体量很大，应基于结构化字段和目标关键词提取，不把记录中的指令当作当前授权。
- 必须区分“出口 IP 地理位置”和“Apple 网络定位坐标”；二者的负责模块与验收证据不同。
- 当前本机 Worker 为单一全局坐标、query token、开放 CORS、KV 异常回落默认坐标，且没有 Git/测试/CI/明确许可证，不能直接生产集成。
- WLOC 上游 AGPL 与 Gateway MIT 存在打包边界；推荐保持独立分发与协议/链接集成。
- Shadowrocket 必须在 WLOC 坐标生效并确认后关闭，否则可能与路由器 TPROXY 双重代理冲突。
- 新方案改为不依赖 Shadowrocket，但引入路由器 CA、TLS/HTTP2 MITM、protobuf patch、精确流量隔离和 AGPL/MIT 边界，开发风险显著高于 LuCI 功能。
- PoC 必须在协议 fixture、fail-open、CA 权限和普通 HTTPS 不受影响的测试完成后才能连接真机。
- 针对正确方案的安全复核评分为 58/100；独立 nft table、IPv6/AAAA、真实 fixtures、CA 生命周期和精确 fail-open 是进入 MITM 阶段的阻断门禁。

## 多 Agent 与 GitHub 基线

- 当前工作区本身是一个尚无首个提交的 Git 仓库，适合作为独立 `wificalling-location-gateway` 仓库初始化。
- GitHub CLI 已登录用户 `smthdagg`，token 具备私有仓库和 workflow 所需 scope；不需要创建或写入新凭据。
- `smthdagg/wificalling-location-gateway` 当前不存在，创建不会覆盖远程项目。
- 计划推荐 Go，而不是 C + TLS/nghttp2；多 Agent 首批任务应先覆盖协议证据、威胁模型、测试夹具、出口探测和 OpenWrt 隔离设计。
- GitHub Projects v2 不是首期硬依赖；Issue + 标签 + Milestone + PR 已足以形成可审计任务队列，避免额外 project scope 和自动化复杂度。
- GitHub API 明确拒绝为当前账号的私有仓库启用 branch protection（HTTP 403，要求 GitHub Pro 或公开仓库）。这不影响私有仓库、Issues、Actions 和 PR，但服务端无法强制禁止管理员直推。

## 可接管 Agent 模型

- 不同 Agent 使用不同 API Key 时，凭据不应集中保存或进入 Git/GitHub；接管只需要代码、测试证据、环境能力和任务状态。
- GitHub assignee 无法区分共用同一 GitHub 身份但使用不同模型/API Key 的 Agent，因此 Agent ID 与能力必须写入结构化租约/交接记录。
- GitHub Issue 标签更新不是强一致锁；租约属于协作锁。代码连续性的真正锚点是已推送的不可变 commit SHA。
- 最小可靠交接单元应包含：Issue、分支、commit、完成项、未完成项、测试命令/结果、失败记录、下一步、所需能力、环境假设和安全事项。
- 接管 Agent 可以能力不同，但不得绕过任务的 required capabilities；能力不足时可做研究或复核，不能执行受限实现。
