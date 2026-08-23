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

## 开发准备基线

- 当前仓库仍是规划与多 Agent 协作脚手架，未出现 `go.mod`、Makefile 或产品源码；因此不能把现有 Python/Shell 测试当作 WLOC 产品测试。
- 本机已有 Git、GitHub CLI、Python 3.9、Clang、Make、OpenSSL、Docker、Node/npm；缺少 Go、ShellCheck、Gitleaks、ARM64 QEMU 和可发现的 OpenWrt/ImmortalWrt SDK。
- 本地未安装 Python `coverage`；当前仓库测试可以执行，但暂时不能给出 Python 行/分支覆盖率数字。
- 远程 main 最近一次 `repository-gates` 在 `a68bc55` 上成功，可证明当前协作脚手架的 CI 基线健康。
- 产品开发准备必须至少补齐 Go 工具链并建立 `go.mod`/构建入口；OpenWrt SDK 和 ARM64 仿真可以在进入打包/系统验证前补齐，不应阻塞 Phase 0 文档、fixtures 合规取证和纯 Go 协议测试设计。
- 新增准备度检查器将缺失项作为显式非零退出门禁，避免后续 Agent 仅凭自身能力声明跳过仓库级 Phase 0 证据。
- Python 标准库 `trace` 不适合作为本项目的正式覆盖率门禁；本机也没有 `coverage`。当前只能报告测试通过，不能声称达到 80% 行/分支覆盖率。
- 当前最有效的启动顺序是先并行完成许可证 ADR、fixture 治理和威胁模型；三者可评审后再创建 Go module 和协议测试骨架。

## Phase 0 批次补充

- 2026-08-11 官方 Go 下载页显示稳定版本为 Go 1.26.5，Go 1.27 仍处在当月预期发布窗口；项目不应仅为追新把 `go` 指令设成尚未稳定的 1.27。
- `actions/setup-go` 官方仓库支持从 `go.mod` 的 `go`/`toolchain` 指令选择工具链；CI 可用固定 commit 的 setup action，从模块文件读取版本，避免本机版本漂移。
- OpenWrt 官方 packages feed 通过 `lang/golang/golang-package.mk` 构建 Go 包；`openwrt-24.10` 当前明确为 Go 1.23.12，因此 `go.mod` 应声明 Go 1.23 兼容基线，并用 1.23.12 容器/SDK验证，而不是直接跟随桌面端最新 Go。
- 当前 Docker daemon 可用，但没有 Go 镜像；系统没有 Homebrew。若本轮需要本地 Go RED/GREEN，将优先使用固定版本官方 Go 容器并记录镜像版本，而不是运行不受控网络安装脚本。
- GitHub Issues #1、#2、#3 分别与许可证 ADR、fixture 契约、威胁模型完全对应且仍为 `status:ready`；本轮只做本地并行交付，不改变远程 Issue 状态。
- 只读源码的 Go 容器仍需要可执行的临时构建缓存；本机 Docker 的 `--tmpfs /tmp` 路径即使去掉显式 `noexec` 仍不能执行测试二进制。权威 fallback 使用独立临时 cache bind，源码保持 `:ro`、网络保持关闭。
- 通用 `protocolgate` 只做 metadata/resource prefilter；它不执行 WLOC hostname scope。未来 WLOC wrapper 必须把两个批准 hostname 固化为可信编译期常量，不能从 SNI、Host、配置或请求数据动态生成 allowlist。
- `InspectCandidate` 只表示资源元数据进入后续审查候选，绝不表示协议已识别或允许 parse/patch；空 body、未知 host、无 allowlist、非法/超限大小和倍率均强制 `PassThrough`。
- Go CI 当前实行零外部依赖 `GOPROXY=off` 策略。未来引入 protobuf/H2 等模块前必须选择 vendor 或单独的固定、可审计依赖获取阶段，不能在测试时隐式联网。

## Rust 技术路线审计

- OpenWrt `openwrt-24.10` 官方 packages feed 的 `lang/rust/Makefile` 声明 `PKG_VERSION:=1.90.0`，目标配置通过 `TARGET_CC_NOCACHE`/`TARGET_CXX_NOCACHE`/`TARGET_AR`/`TARGET_RANLIB` 注入交叉工具链，并在 musl 配置下设置 `musl-root=$(TOOLCHAIN_ROOT_DIR)`。来源：https://raw.githubusercontent.com/openwrt/packages/openwrt-24.10/lang/rust/Makefile
- Rust 官方 `aarch64-unknown-linux-musl` 目标是 Tier 2，适用于 64-bit little endian ARMv8-A Linux + musl；该目标通过 rustup 分发并可从任意 host 交叉编译，但构建目标本身或含 C/ASM 依赖时需要 `aarch64-linux-musl-gcc`/`g++`/`ar`/linker 在 PATH 或构建配置中可用。来源：https://doc.rust-lang.org/rustc/platform-support/aarch64-unknown-linux-musl.html
- `rustls 0.23.43` 文档声明默认使用 `aws-lc-rs`，可通过 `default-features = false` 移除隐式 `aws-lc-rs`；内置 provider 包括默认 `aws-lc-rs` 和可选 `ring`。`rustls` MSRV 为 Rust 1.71，启用可选 `zlib-rs` 时需要 1.75。来源：https://docs.rs/rustls/latest/rustls/ 与 https://docs.rs/crate/rustls/latest/features
- `tokio 1.53.1` 默认不启用任何 feature；`full` 会启用 fs、io-std、process、rt-multi-thread、signal 等不适合 AX6S 最小 PoC 的能力。最小网络 runtime 应显式只启用 `rt`、`net`、`io-util`、`time`，测试需要宏时才启用 `macros`。来源：https://docs.rs/crate/tokio/latest/features
- `h2 0.4.15` 是 Tokio aware 的 HTTP/2 client/server 实现，依赖 `tokio` 与 `tokio-util`，但文档明确不负责 TCP、TLS、HTTP/1 upgrade 或非 HTTP/2 特性；TLS ALPN 和连接准备必须由本项目显式处理。来源：https://docs.rs/crate/h2/latest 与 https://docs.rs/h2/latest/h2/
- `prost 0.14.4` 当前 MSRV 为 Rust 1.85，兼容 OpenWrt 24.10 的 Rust 1.90.0；`prost-build` 自 v0.11 起需要外部 `protoc`，因此 OpenWrt 构建路径应优先提交生成后的 Rust 类型或在离线工具阶段固定 `protoc`，避免目标包构建时动态生成。来源：https://docs.rs/prost/latest/prost/
- 本机初始缺少 Linux musl target/linker；当前 PATH 仍未发现 `aarch64-unknown-linux-musl` target、`aarch64-linux-musl-gcc` 或 `aarch64-openwrt-linux-musl-gcc`。
- 修正后 Rust spike 的 locked native release 为 951,504 bytes。曾有离线 OpenWrt AArch64 stripped 产物 1,118,872 bytes 的记录，但当前工作树未保留复现脚本、日志或产物；该数值只能作为待复验线索，不能作为主审计正式放行证据。
- `cargo-audit 0.22.2` 未发现 RustSec 漏洞；`cargo-deny 0.20.2` 的 advisories/bans/licenses/sources 全部通过，且项目本身保持尚未授权状态。
- `cargo tree -e features --locked` 显示 `ring 0.17.14` 通过 build-dependency `cc 1.4.2` 编译 native 代码；这比纯 Rust 依赖更依赖 OpenWrt SDK 的 C toolchain，可接受但必须作为 ARM64 spike 的显式风险项。
- 本仓库当前权威 host 范围来自 `DEVELOPMENT_TEST_PLAN.md` 与 `docs/security/WLOC_THREAT_MODEL.md`：`gs-loc.apple.com` 和 `gs-loc-cn.apple.com`。早前历史摘要中的其他 hostname 不作为本轮实现依据。

## Rust 路线审计

- OpenWrt 官方 `packages` 的 `openwrt-24.10/lang/rust/Makefile` 当前固定 Rust 1.90.0；因此项目 MSRV 必须不高于 1.90.0，不能以本机 Rust 1.97.1 能编译作为 OpenWrt 兼容证据。
- Rust 官方将 `aarch64-unknown-linux-musl` 列为 Tier 2，支持从任意 host 交叉编译；涉及 C/ASM 的 crate 仍需 `aarch64-linux-musl-gcc` 等目标工具。
- 桌面 host 仍无直接 aarch64-musl linker，但已用受控 Linux 容器中的 OpenWrt 官方工具链取得真实 AArch64 构建证据。
- rustls 0.23.43 默认启用 aws-lc-rs、logging、post-quantum 偏好、std 和 TLS 1.2；资源 Spike 必须关闭 default features 并明确选择 crypto provider，否则体积与交叉构建成本会被默认依赖放大。
- 已安装固定版本 `cargo-audit 0.22.2` 与 `cargo-deny 0.20.2`，并将漏洞、license、source 与重复依赖检查接入 Rust verifier/CI。
# 2026-08-23 Issue #66 Standard/Lite release findings

- 用户确认这不是项目分叉，而是同一项目的不同内存版本；Git 分支仅为临时开发/审核机制。
- 现有“三平台”在发布脚本中实际是三类安装产物：AArch64 cortex-a53 IPK、x86_64 OpenWrt/iStoreOS 24.x IPK、x86_64 OpenWrt 25.x APK。Docker 运行矩阵额外把同一个 x86_64 IPK 分别装入 OpenWrt 与 iStoreOS，因此是 3 个资产目标、4 个运行环境。
- 当前稳定链是 `origin/main -> Issue #62 -> Issue #64`；Issue #64 分支可从 main 快进，故 Issue #66 已从 main 建立并快进到 `81d5f6b`，没有混入 v2/Beta 代码。
- 当前 standalone 和 SDK builder 都把 `sing-box` 写为硬依赖，健康脚本和运行脚本还硬编码 `/usr/bin/sing-box`。双变体实现必须先抽象运行时路径，再区分系统运行时与 tiny payload。
- Lite 通过包管理器显式 `Provides/Replaces/Conflicts` 接管 sing-box 契约，不能与 Standard 或独立 sing-box 包共存；`/usr/bin/sing-box` 是透明包装器，WCG 与 PassWall 共享经哈希校验的 `/tmp/sing-box-lite`，不是静默覆盖未知系统文件。
- 首个直接写入 29.7 MB ELF 的 AX6S 候选使 overlay 仅余 2.8 MB，已阻断并废弃。压缩驻留修正版清理旧重复文件后恢复到约 20.4 MB 可用，并通过冷启动。
- AX6S 冷启动后实际记录到 `192.168.31.175` 请求 `gs-loc.apple.com/clls/wloc`，WLOC 响应已成功生成，状态保持 `intercepting`。
- 版本切换必须用 package conflict/provides/replaces 与 conffiles 共同保证“不可共存但配置不丢失”；安装、升级、降级和回滚都需要离线测试。
