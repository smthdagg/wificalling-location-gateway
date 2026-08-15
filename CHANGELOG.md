# Changelog

All notable changes are documented here. Versions follow Semantic Versioning.

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
