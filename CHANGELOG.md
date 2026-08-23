# Changelog

All notable changes are documented here. Versions follow Semantic Versioning.

## [1.2.2-r4] - 2026-08-23

Stable deleted-node fail-closed hotfix and release-baseline isolation.

### Fixed

- **No arbitrary node fallback**: auto-follow now requires the selected WCG
  device and its bound node to exist in the current UCI configuration. A
  deleted binding can no longer fall through to the first sing-box outbound,
  endpoint, or a stale generated route.
- **No stale location after binding loss**: when the followed node is missing,
  the runtime clears the exit IP and Geo result and reports an actionable
  error instead of retaining a previous country, city, or coordinate.
- **Visible refresh result**: the WLOC monitor refresh action now waits for the
  daemon result and reports changed, unchanged, or unavailable exit state.
  The new messages and missing-node reason include Chinese translations.

### Release boundary and verification

- Both formal package builders now accept only SHA-256-pinned stable integrated
  `wificalling-location-gateway` 1.2.x packages as their baseline. Retired
  standalone package lines are rejected.
- Project rules exclude the independently maintained multi-device/2.0 Beta
  line from this repository. The IPK `debian-binary` value `2.0` remains only
  as the required archive-format marker.
- Built and verified the AX6S AArch64 IPK, OpenWrt/iStoreOS 24.x x86_64 IPK,
  and OpenWrt 25.12 x86_64 native APK across four pinned rootfs environments.
- Installed the exact AArch64 R4 asset on Redmi AX6S after removing R3 to fit
  the constrained overlay. Both UCI files were preserved. A temporary missing
  binding produced unavailable exit/Geo state with no unrelated fallback;
  restoring the configuration returned the exit state to verified.

### 中文说明

- 修复自动跟随设备的原绑定节点被删除后，程序错误选择首个可用节点的问题。
- 绑定节点不存在时立即清除旧出口 IP 与旧地理信息，并提示重新选择、应用 WCG
  节点；刷新按钮现在明确显示出口已变化、未变化或不可用。
- 正式构建基线严格限制为已校验哈希的 1.2.x 稳定整合包，拒绝 1.7 等退役独立包。
- 多设备/2.0 Beta 已从本项目范围移除并由独立项目维护；IPK 内的 `2.0` 仅为包格式。
- 三个平台包与四环境 Docker 矩阵通过，R4 已在 AX6S 按低存储流程安装并完成回归。

## [1.2.2-r3] - 2026-08-23

Stable runtime hotfix and completion of the three-platform release set.

### Fixed

- **Non-blocking TPROXY accept path**: the WLOC listener no longer blocks the
  async runtime while accepting redirected connections. AX6S validation
  completed five independent local TLS/HTTP2 WLOC requests with HTTP 200.
- **Validated local WLOC response path**: authorized WLOC requests use the
  configured local response path again instead of timing out before protocol
  processing.
- **Accurate upstream failure health**: upstream connection failures now reach
  the proxy-health recorder instead of being hidden as a healthy state.
- **Monitor temporary-file leak**: monitor updates no longer leave zero-byte
  temporary files behind on the router.
- **Stable integrated package input**: the release builder now accepts the
  SHA-256-pinned `wificalling-location-gateway` 1.2.x package identity and the
  `-rN` revision format. It still rejects unrelated package names and versions.

### Packaging and verification

- Published one integrated package per target: AX6S AArch64 IPK, OpenWrt /
  iStoreOS 24.x x86_64 IPK, and OpenWrt 25.12 x86_64 native APK v3.
- Passed install/start/socket/status checks in four pinned rootfs environments:
  OpenWrt 24.10.5 AArch64, OpenWrt 24.10.8 x86_64, iStoreOS 24.10.5 x86_64,
  and OpenWrt 25.12.3 x86_64.
- Re-verified the release AArch64 package on a Redmi AX6S: WCG, sing-box, WLOC,
  generated configuration, and the WLOC control socket were healthy; core
  service PIDs remained stable during observation.

### 中文说明

- 修复 WLOC TPROXY 接收路径阻塞、上游失败状态遗漏和监控临时文件泄漏。
- 恢复并验证本地 WLOC 响应链路；AX6S 上 5 次 TLS/HTTP2 请求均返回 HTTP 200。
- 补齐 AX6S AArch64 IPK、OpenWrt/iStoreOS 24.x x86_64 IPK 与 OpenWrt 25.12
  x86_64 原生 APK 三个平台安装包，并完成四环境 Docker 与 AX6S 真机验证。
- 修复正式打包器，使其可直接校验并复用稳定整合包，不再依赖临时篡改包元数据。

## [1.2.2] - 2026-08-19

Manual-mode exit-probe fix, compiler endpoint filtering, and LuCI follow-device logic corrections.

### Changed

- **Manual mode no longer probes the exit IP** (`src/app.rs`): exit probing
  only drives auto-follow, so in manual mode it is skipped entirely and the
  status `exit` block reports no IP. The LuCI monitor shows
  "Manual mode - not applicable" for the Exit IP row.
- **Compiler filters unreferenced WireGuard endpoints too**: `compiler.sh`
  now skips endpoints whose node is not referenced by a device policy
  (`used[]` check with `n_wg_used` counting), so sing-box.json keeps only
  nodes actually in use; the empty endpoints block is no longer emitted.
- **LuCI Follow device fixes** (`wloc.js`): the dropdown now treats
  `source_ip` as a DynamicList (array) - previously an array was used as
  the value and selecting a device could write a wrong `assigned_device`;
  and only ENABLED devices are listed (a disabled device's node is
  filtered from sing-box.json, so following it silently probed a fallback
  node). The monitor warns "disabled! Enable it to follow its node" when
  the followed device is disabled.
- **Mode switch is persisted immediately** (`wloc.js`): switching
  auto/manual now saves `geo_source` right away, so a later Follow-device
  save can no longer carry a stale pending mode value over and flip the
  service back to manual.
- **LuCI monitor syntax fix** (`wloc-monitor.js`): the Exit IP cell was
  rewritten with a plain `exitVal` variable (a nested ternary had one
  extra `(` that broke the whole page with `SyntaxError: missing )`).

## [1.2.1] - 2026-08-19

Compiler optimization: only load proxy nodes that are actually referenced by device policies.

### Changed

- **Compiler node filtering**: `compiler.sh` now tracks which nodes are
  referenced by active device policies and only compiles those into
  `sing-box.json`. Unreferenced nodes are excluded from the outbound
  list and WireGuard endpoint block. Verified on AX6S with 10 configured
  nodes (7 WireGuard) and 1 active device: the generated sing-box.json
  shrank from 2827 to 1534 bytes, and only the referenced vmess node
  plus `direct` remain as outbounds.
- **Reduced sing-box memory footprint**: on a Redmi AX6S the
  wificalling-gateway sing-box instance RSS dropped from ~19-23 MB to
  ~15 MB (16 → 9 threads) because unused WireGuard tunnels and protocol
  stacks are no longer loaded; system available memory rose to ~29 MB.
- **Packaging fix**: the project now ships its own complete
  `init.d/wificalling-gateway` (full 24-field normalized.conf pipeline,
  `wireguard_style` detection) and a clean LF `compiler.sh`, so the
  integrated package no longer depends on the older 19-field init.d
  bundled in the gateway 1.2.x IPK. Verified via fresh install on AX6S
  (sing-box check passes, normalized.conf emits 25 fields) and the
  four-environment Docker matrix.
- **README housekeeping**: removed outdated "Wi‑Fi Calling Gateway 1.7"
  references throughout; the project is now a single integrated package.
  Updated release badge, install commands, build examples, and the
  GitHub Linguist snapshot to match 1.2.1.

## [1.2.0] - 2026-08-17

WireGuard node reliability and connection testing overhaul.

### Added

- **Per-node "nodeTest" connection test button** on the settings page: every
  proxy node row now has a test button in front of Edit/Delete. WireGuard
  nodes run a fresh handshake on demand (bypassing the monitor loop's 60s
  result cache) and report the verified exit IP; all other protocols get a
  TCP reachability probe of server:port. Results appear in a banner with an
  explicit close button that never auto-dismisses.
- **Handshake failure reasons**: the node status export and the LuCI view
  now classify failed WireGuard handshakes (`config_missing` / `timeout` /
  `unreachable`) instead of a bare "Offline". The status cell shows a short
  label (e.g. "Handshake failed (Timeout)") and the full explanation is
  available as a tooltip.

### Changed

- **WireGuard handshake probe hardened**: the probe now forwards the node's
  `reserved` field (WARP-style endpoints), derives its probe port from an
  md5sum hash instead of id arithmetic (cksum is absent on ImmortalWrt
  busybox and crashed the whole monitor script), and serializes concurrent
  monitor ticks with a mkdir lock that reclaims locks left by killed
  processes - two racing ticks previously handed each other the wrong exit
  IP.
- **Compact node table**: only the essentials are shown as columns - Name,
  Enable, Protocol, Server, Port, Node status, Ping/latency, Quality and
  the WireGuard preshared key. Every other field (password, uuid, TLS,
  Reality, transport, key material, ...) is hidden from the table and stays
  editable in the per-node modal.

### 中文说明

- **新增**：设置页每个节点新增「nodeTest」连接测试按钮（位于 Edit/Delete 之前）。WireGuard 节点点击后立即执行一次真实握手（绕过监控循环 60 秒结果缓存）并显示出口 IP；其他协议节点执行 TCP 连通性探测。测试结果以带显式关闭按钮的横幅展示，不会自动消失。
- **新增**：握手失败原因分类（配置缺失 / 超时 / 不可达）——节点状态不再只是笼统的「离线」，状态列显示简短标签（如「握手失败 (超时)」），完整原因可通过悬停查看。
- **改进**：WireGuard 握手探测加固——转发 reserved 字段（WARP 类节点必需）；探测端口改用 md5sum 哈希派生（ImmortalWrt busybox 没有 cksum，旧实现会导致整个监控脚本崩溃）；用 mkdir 锁串行化并发监控轮询并回收被中断进程遗留的锁（此前两个并发轮询会互相写错出口 IP）。
- **改进**：节点表格精简——仅保留 Name、启用、协议、服务器、端口、节点状态、Ping/延迟、质量、WireGuard 预共享密钥列；其余字段（密码、UUID、TLS、Reality、传输方式、密钥材料等）从表格隐藏，仍在编辑弹窗中可改。

## [1.0.11] - 2026-08-16

### Changed

- **Service Status page**: the Wi-Fi Calling Gateway block and its restart
  button now come first (matching the submenu order), followed by WLOC.
- **One-click service restarts** on the Service Status page: "Restart
  Wi-Fi Calling gateway" regenerates the proxy config and restarts
  sing-box; "Restart WLOC service" restarts the daemon. The health report
  refreshes immediately after each restart.
- **Proxy config age clarified**: a large age is normal (the config is
  only regenerated on gateway restart), so the page shows "generated N s
  ago" and only warns when nodes/devices were changed without a restart.

### 中文说明

- **改进**：服务状态页顺序调整为 Wi-Fi 通话网关在前、WLOC 在后（与子菜单一致），重启按钮同步。
- **新增**：服务状态页一键重启按钮——「重启 Wi-Fi 通话网关」会重新生成代理配置并重启 sing-box；「重启 WLOC 服务」重启守护进程；重启后健康报告立即刷新。
- **改进**：代理配置年龄语义澄清——配置只在网关重启时重新生成，数值大属正常，页面改为显示「N 秒前生成」，仅当节点/设备变更而未重启时给出提示。

## [1.0.10] - 2026-08-16

### Changed

- **Service Status page polished**: the menu entry moved to the end of the
  submenu (right before Help), the bulky status badges were replaced with
  compact colored dots, label/value rows are tight (no stretched columns),
  and the redundant log tail was removed - the per-service monitor pages
  already show logs.
- README features and the in-app FAQ document the Service Status page
  (both English and Chinese).

### 中文说明

- **改进**：服务状态页面打磨——菜单入口移到子菜单末尾（紧挨帮助之前）；大块状态徽章改为紧凑彩色圆点；标签与状态值紧凑排列（不再被拉宽）；删除了重复的最近日志区块（各监控页已有日志）。README 功能列表与页面内 FAQ 已补充服务状态页说明（中英双语）。

## [1.0.9] - 2026-08-16

### Added

- **Dedicated "Service Status" page** (Services > Service Status): a single
  place to see whether both services are healthy, refreshed every 10 s:
  - WLOC service: daemon process, control socket, status-file freshness,
    service phase, exit probe and geo resolution state, last error.
  - Wi-Fi Calling Gateway: monitor loop and sing-box processes, proxy
    config presence/validity/age, normalized config freshness, nftables
    rule count, device policies.
  - Build patches (WireGuard PSK / handshake check / compact node status /
    stale device guard) and node health (total/online/offline/unknown).
  - Tail of the wloc-service, gateway and sing-box logs.
  Backed by `/usr/sbin/wloc-health.sh`, exposed through the `luci.wloc`
  rpcd `health` method.

### 中文说明

- **新增**：「服务状态」专门页面（服务 > 服务状态）：集中查看两个服务是否正常，每 10 秒自动刷新：
  - WLOC 服务：守护进程、控制套接字、状态文件新鲜度、服务阶段、出口探测与定位解析状态、最后错误。
  - Wi-Fi 通话网关：监控循环与 sing-box 进程、代理配置是否存在/有效/生成时间、节点清单新鲜度、nftables 规则数、设备策略数。
  - 构建补丁（WireGuard 预共享密钥 / 握手检查 / 节点状态精简 / 设备引用守卫）与节点健康（总数/在线/离线/未知）。
  - wloc-service、网关与 sing-box 的最近日志。
  数据由 `/usr/sbin/wloc-health.sh` 生成，经 `luci.wloc` rpcd 的 `health` 方法提供给页面。

## [1.0.8] - 2026-08-15

### Fixed

- **A stale device-policy node reference no longer stops the whole
  gateway**: re-importing nodes in LuCI changes their UCI section names;
  a device policy that still referenced an old name made the config
  compiler fail outright, so sing-box.json was never generated and the
  proxy stopped for every device (all traffic fell back to direct - a
  test site showed the router gateway IP instead of the node exit). The
  compiler now skips such devices with a warning: the stale device routes
  directly while every other device keeps proxying.

### 中文说明

- **修复**：设备策略引用已不存在的节点不再导致整个网关失效——在 LuCI 里重新导入节点会改变节点的 UCI section 名称，若某条设备策略仍引用旧名称，配置编译器会直接失败，sing-box.json 无法生成，所有设备的代理全部停止（流量全部直连——测试站点显示路由器网关 IP 而不是节点出口）。现在编译器会跳过这类设备并输出警告：过期引用的设备走直连，其余设备继续正常代理。

## [1.0.7] - 2026-08-15

### Fixed

- **WLOC follow-device exit probe now works for WireGuard nodes**: the
  probe only looked for the bound node among the Gateway sing-box
  outbounds, but sing-box 1.13 keeps wireguard peers in `endpoints` (named
  `wg-<section>`), so a device bound to a WireGuard node silently fell
  back to the first regular outbound (or direct) and the monitor showed
  the router WAN IP instead of the node exit - the follow-device location
  never updated to the node's real country/city. The probe now parses and
  re-emits wireguard endpoints (`route.final` naming, same shape as the
  node-health handshake probe) and resolves the UCI binding to the
  `wg-<section>` tag. Verified live: a WireGuard node exit now reports
  GB / Maidenhead / Europe/London with the exit IP in the monitor.

### 中文说明

- **修复**：WLOC「跟随设备」出口探测现在支持 WireGuard 节点——之前探测只在 sing-box 的 outbounds 里找设备绑定节点，而 sing-box 1.13 把 wireguard 对端放在 `endpoints`（命名为 `wg-<section>`），导致绑定 WireGuard 节点的设备静默回退到其他节点或直连，监控页显示的出口 IP 是路由器 WAN IP 而不是节点出口，跟随定位永远更新不到节点的真实国家/城市。现在探测会解析并复用 wireguard endpoints（`route.final` 指向 endpoint，与节点握手探测同款配置），并把 UCI 绑定正确解析为 `wg-<section>` tag。已真机验证：WireGuard 节点出口现在在监控页显示 GB / Maidenhead / Europe/London 及出口 IP。

## [1.0.6] - 2026-08-15

### Audit

- Full audit and verification pass over all components. No functional
  defects found in this cycle; the release repackages and re-signs the
  verified tree:
  - Rust: 99 unit tests, `clippy` clean, `rustfmt` clean, dependency
    advisories/bans/licenses green.
  - Python: 69 tests; LuCI JS regression tests (tab localization, mode
    switch, secondary-router fallback) all pass; packaging scripts
    ShellCheck-clean.
  - Install artifacts: four-environment Docker matrix
    (AX6S aarch64, OpenWrt 24.10/25.12 x86_64, iStoreOS) installs, starts,
    and passes socket/status checks; SHA256SUMS verified against the
    manifest.
  - Live Redmi AX6S (ImmortalWrt 24.10.6): services up, CA profile
    served, node status document valid with all nodes reporting
    (reachable / handshake OK with exit IP / handshake failed), control
    API (status/refresh/geo-set) responsive, all three build patches
    present.
- Privacy sweep: no real device IPs, credentials, or keys in the
  repository (the only LAN IP is a unit-test fixture).

### 中文说明

- **审计**：本轮对所有组件做了全面审计与验证，未发现功能性缺陷；本版本对验证通过的全部代码重新打包并签名发布：
  - Rust：99 个单元测试、`clippy` 零警告、`rustfmt` 通过、依赖安全（advisories/bans/licenses）全绿。
  - Python：69 个测试；LuCI JS 回归测试（标签本地化、模式切换、旁路由兜底）全部通过；打包脚本 ShellCheck 干净。
  - 安装产物：四环境 Docker 矩阵（AX6S aarch64、OpenWrt 24.10/25.12 x86_64、iStoreOS）安装、启动、socket/status 检查全部通过；SHA256SUMS 与清单一致。
  - Redmi AX6S 真机（ImmortalWrt 24.10.6）：服务运行正常、证书描述文件可下载、节点状态文档有效且所有节点状态正确（在线 / 握手成功带出口 IP / 握手失败）、控制接口（status/refresh/geo-set）正常、三个构建补丁全部落地。
- **隐私**：仓库内无真实设备 IP、凭据或密钥（唯一的局域网 IP 是单元测试 fixture）。

## [1.0.5] - 2026-08-15

### Fixed

- **Node status column blank after a WireGuard handshake**: the node health
  document embedded the WireGuard exit IP unquoted (`"ping_ms":1.2.3.4`),
  which made the whole `node-status.json` invalid JSON - LuCI's parse failed
  and every Node status / Ping / Quality column rendered as "-" once any
  WireGuard node completed a handshake. The exit IP is now quoted.
- **Compact status document + static export**: the status document only
  carries the fields the LuCI view reads (id/state/measurement/ping_ms) and
  is exported under the uhttpd docroot (`/www/wloc-node-status.json`), so
  the view reads it with a plain GET - immune to the `/ubus` JSON-RPC reply
  truncation seen on some firmwares (ImmortalWrt 24.10) with larger
  replies. The quality threshold comparison now uses `parseFloat` to accept
  both numeric and quoted ping values.

### 中文说明

- **修复**：WireGuard 握手成功后节点状态列空白——节点健康检查把握手出口 IP 以无引号形式写进 `node-status.json`（`"ping_ms":1.2.3.4`），导致整个文件不是合法 JSON，LuCI 解析失败后所有节点的「节点状态 / Ping / 质量」列都显示 "-"。现在出口 IP 会正确加引号。
- **新增**：状态文档精简并静态导出——`node-status.json` 只保留 LuCI 界面需要的字段（id/state/measurement/ping_ms），并导出到 uhttpd 文档根目录（`/www/wloc-node-status.json`），页面用普通 GET 读取，不受部分固件（如 ImmortalWrt 24.10）上 `/ubus` JSON-RPC 大响应截断的影响。质量阈值比较改用 `parseFloat`，同时兼容数字和带引号的 ping 值。

## [1.0.4] - 2026-08-15

### Fixed

- **Secondary router (no DHCP) support**: the device-policy status column no
  longer reports every device as offline when `/tmp/dhcp.leases` is empty. A
  device recently seen in the ARP cache is shown as "Online (static IP)", so
  routers that do not run DHCP (secondary/AP routers) now work as expected;
  the FAQ documents the setup (point the device gateway at the secondary
  router).
- **CA profile always available**: `wloc-service` exports the iOS CA profile
  (`/www/wloc-ca.mobileconfig`) automatically right after the root CA is
  ready, and the WLOC settings page auto-(re)generates it on every load. The
  profile link now works without clicking "Regenerate profile", and when
  generation fails the page shows the reason instead of a 403/404 dead link
  (older uhttpd builds answer missing files with 403).
- **WireGuard handshake validation**: node health checks validate WireGuard
  nodes by a real handshake and report the verified exit IP, instead of
  relying on ICMP alone.

### 中文说明

- **新增**：支持旁路由（不提供 DHCP）——设备策略状态列不再因 `/tmp/dhcp.leases` 为空而把所有设备误报为「设备离线」：设备只要近期出现在 ARP 邻居表就显示「在线（静态 IP）」，旁路由/AP 模式可正常使用；FAQ 已补充旁路由设置说明（把设备网关指向旁路由 IP）。
- **新增**：证书描述文件自动生成——`wloc-service` 在根证书就绪后自动导出 `/www/wloc-ca.mobileconfig`，WLOC 设置页每次加载也会自动重新生成；证书链接无需手动点击「重新生成描述文件」即可使用，生成失败时页面会显示具体原因，而不是 403/404 死链接（旧版 uhttpd 对不存在的文件返回 403）。
- **新增**：WireGuard 节点真实握手校验——节点健康检查通过真实握手验证 WireGuard 节点并上报出口 IP，不再只依赖 ICMP。

## [1.0.3] - 2026-08-15

### Added

- `star-history-chart` workflow regenerates `docs/images/star-history.svg` daily
  from the official GitHub stargazer API using the auto-injected Actions token
  (no third-party service, no token in the repository); README embeds the
  committed chart instead of the star-history.com embed.
- **WireGuard pre-shared key support** for the integrated Gateway: the build
  pipeline patches the merged Gateway payload (`compiler.sh` + `init.d`) so
  a `pre_shared_key` UCI option is emitted into the sing-box wireguard peer
  block (endpoint and legacy styles), and the LuCI node editor gained a
  "WireGuard preshared key" field. Nodes that require a PSK now complete the
  handshake (verified against live nodes: handshake in ~1s, exits in the UK).
- **Standard WireGuard config import**: the "Import proxy node" dialog now
  accepts a full `[Interface]`/`[Peer]` config block in addition to `wg://`
  share links, mapping PrivateKey, Address, MTU, Reserved, peer PublicKey,
  PresharedKey, and Endpoint onto the node fields with clear validation.
- The README now leads with the English introduction followed by the Chinese
  version; the release notes are bilingual (English first, Chinese after).

### 中文说明

- **新增**：`star-history-chart` 工作流每日用 GitHub 自动注入的 token 重新生成 `docs/images/star-history.svg`（不经过第三方服务，token 不写入仓库）；README 嵌入该图表。
- **新增**：集成网关支持 WireGuard 预共享密钥——构建流程自动给合并的网关载荷打补丁（`compiler.sh` + `init.d`），`pre_shared_key` UCI 选项会写入 sing-box 的 wireguard peer（endpoint 与 legacy 两种格式），LuCI 节点编辑器新增「WireGuard 预共享密钥」字段；需要 PSK 的节点现在可以完成握手（已用真实节点验证：握手约 1 秒，出口在英国）。
- **新增**：「导入代理节点」支持标准 WireGuard 配置——除了 `wg://` 分享链接，现在可以直接粘贴整段 `[Interface]`/`[Peer]` 配置块，自动提取 PrivateKey、Address、MTU、Reserved、对端 PublicKey、PresharedKey 和 Endpoint，缺字段给出明确报错。
- **新增**：README 改为英文介绍在前、中文版本在后；发布说明双语（英文在前、中文在后）。

## [1.0.2] - 2026-08-15

Router package update (v1.0.2-2) fixing node-switch follow-up and adding a
manual refresh for the WLOC monitor.

### Added

- **Manual refresh IP button** on the WLOC Monitor page, next to the followed
  device: it discards cached probe evidence, re-probes the followed node
  immediately, and rewrites the status file so the exit IP updates on the spot
  (new `control.refresh` control command wired through `wloc-ctl` and the
  `luci.wloc` rpcd bridge; a failed probe shows the reason without restarting
  the daemon).
- **Connected-device picker in the "Add LAN device" form**: the edit modal
  lists the LAN devices detected from the DHCP leases and the ARP cache
  (hostname + real IP, router and already-bound IPs excluded); picking one
  fills in the device name and the LAN IP field with the device's actual
  address. The IP placeholder now shows the router's real subnet pattern
  instead of a hardcoded 192.168.31.x.
- The monitor page ships under a versioned LuCI view name like the settings
  page, so an updated page is never served from the browser cache.

### Fixed

- **Node switches are now followed within seconds.** Previously the service
  only re-probed on its 600s housekeeping tick and the config fingerprint
  covered only the running `sing-box.json`, which the Gateway does not
  necessarily rewrite on a binding change - the monitor kept showing the old
  exit IP for up to ten minutes.
  - The fingerprint now also covers the device-policy UCI file
    (`/etc/config/wificalling-gateway`), so any binding change triggers an
    immediate re-probe.
  - The probe selects the node from the UCI device policy first (the user's
    source of truth) instead of trusting possibly stale sing-box route rules.
  - Housekeeping runs every 10 seconds; the probe itself still only runs when
    the fingerprint changed or cached evidence is stale.
- **The router LAN address is no longer hardcoded to 192.168.31.1.** The CA
  profile URL, the DNS hijack, and the matching TPROXY rule now derive the
  router IP from `uci network.lan.ipaddr` (falling back to the `br-lan`
  address), and the LuCI pages build the profile link from the address the
  admin is actually using - so certificate installation and WLOC interception
  work on any LAN subnet, not only 192.168.31.x. The FAQ profile link is now
  a tappable link instead of static text. The packaged export script and the
  daemon's upstream-IP filter use the runtime LAN address as well.
- **The shipped default config no longer pins an example device IP.** A fresh
  install no longer ships `assigned_device 192.168.31.X`; when no follow
  device is chosen in LuCI, the daemon follows the first device policy of the
  Gateway config, so WLOC works out of the box on any subnet.
- The packaged UI no longer lags the repository copy: probe failure reasons
  and the newer translation table were missing from earlier release packages.

### 中文说明

- **新增**：WLOC 监控页“定位跟随设备”旁新增“刷新 IP”按钮——立即丢弃缓存、重新探测跟随节点出口并刷新显示；探测失败显示原因且不会误重启守护进程（新增 `control.refresh` 控制命令，贯通 `wloc-ctl` 与 rpcd `luci.wloc` 桥接）。
- **新增**：“添加局域网设备”弹窗新增“从已连接设备选择”——自动列出 DHCP 租约与 ARP 缓存检测到的局域网设备（设备名 + 真实 IP，排除已绑定 IP 与路由器自身），选择后自动填入设备名称与 IP；IP 输入占位提示改为按路由器实际网段生成，不再写死 192.168.31.x。
- **新增**：监控页与设置页一样使用版本化 LuCI view 名称，升级后浏览器不会缓存旧页面。
- **修复**：切换设备节点后出口 IP 秒级跟随——配置指纹纳入设备策略 UCI 文件、探测优先读取 UCI 设备绑定（而非可能过期的 sing-box 路由规则）、巡检周期缩短至 10 秒（原 600 秒）；原实现最长十分钟不更新。
- **修复**：路由器 LAN 地址不再写死 192.168.31.1——证书链接、DNS 劫持、TPROXY 规则均从 `uci network.lan.ipaddr`（回退 `br-lan`）动态获取，LuCI 页面链接按管理员实际访问地址生成，任意网段（如 192.168.1.x）开箱即用；FAQ 证书链接改为可点击。
- **修复**：出厂默认配置不再写死示例设备 IP（原 192.168.31.X）；未在 LuCI 选择跟随设备时，守护进程自动跟随网关设备策略中的第一台设备。
- **修复**：发布包 UI 与仓库代码保持一致（probe 失败原因显示等此前缺失内容已补齐）。

## [1.0.1] - 2026-08-14

### Fixed

- WLOC monitor now follows the followed device's node immediately when the
  node is switched in the Wi-Fi Calling settings (probe-config fingerprint
  detection instead of waiting for the 300s cache).
- The monitor shows the probe failure reason (node DNS resolution failed /
  connection timed out / unreachable) instead of a bare empty exit IP.
- The exit probe no longer deadlocks on bad nodes whose sing-box output
  never ends (kill the probe child before draining its stderr).

## [1.0.0] - 2026-08-13

### Added

- One integrated, project-named package containing Wi-Fi Calling Gateway 1.7,
  the Rust WLOC service and control client, and unified LuCI/rpcd management.
- AX6S AArch64/cortex-a53 IPK plus x86-64 IPK for OpenWrt/iStoreOS 24.x and
  native APK v3 for OpenWrt 25.x.
- Automatic node-following and manual location modes, CA/profile lifecycle,
  bounded TLS-over-HTTP/2 handling, exit/Geo resolution, and status logging.
- Reproducible pinned builds, SHA-256 release manifests, dependency/license
  auditing, secret scanning, coverage gates, and four-environment Docker smoke
  verification covering every release asset.

### Fixed

- Automatic-to-manual WLOC mode switching now uses the live control socket and
  returns a controlled error without losing the saved configuration.
- Reinstall and upgrade preserve both `/etc/config/wificalling-gateway` and
  `/etc/config/wloc-service`; users no longer need to remove either component.

### Safety

- WLOC interception remains isolated from UDP 500/4500 and the Gateway table.
- Invalid Geo/protocol/TLS state never produces a default fake coordinate.

[1.0.3]: https://github.com/smthdagg/wificalling-location-gateway/releases/tag/v1.0.3
[1.0.2]: https://github.com/smthdagg/wificalling-location-gateway/releases/tag/v1.0.2
[1.0.1]: https://github.com/smthdagg/wificalling-location-gateway/releases/tag/v1.0.1
[1.0.0]: https://github.com/smthdagg/wificalling-location-gateway/releases/tag/v1.0.0
