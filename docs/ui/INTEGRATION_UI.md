# WiFi Calling Gateway + WLOC LuCI 管理方案

Status: implementation contract and historical design record (2026-08-22)
Target: ImmortalWrt 24.10 / LuCI JS。
> 产品边界（2026-08-22）：**本项目是独立的 WiFi Calling Gateway + WLOC 项目**。
> 两个模块在同一仓库、同一 IPK、同一统一生命周期和同一 LuCI 管理面中交付；不
> 安装或依赖外部 Wi-Fi Calling Gateway 1.7 仓库。

## 1. 目标与范围

在 AX6S 的 LuCI 管理界面中，为独立 `wloc-service` 提供完整管理面，提供：

1. **证书获取**：通过 Safari 一键安装/重装 WLOC 根证书（mobileconfig）。
2. **定位模式切换**：自动（跟随节点）/ 手动，随时可切。
3. **WLOC 模块开关**：启用/禁用拦截（不拆 nft 重定向规则，由服务状态机控制）。
4. **当前定位地理信息**：显示当前生效的国家/城市（只读，来自状态快照）。
5. **完整 GPS 数值**：管理员可见的当前坐标（lat/lon），隐私边界仅限本地管理员。
6. **手动定位搜索与保存**：搜索地名（Nominatim 在线 geocode）或直接输入坐标，
   保存为手动预设（持久化，重启不丢）。
7. **独立设备档案**：节点、自动跟随、手动定位、启用、健康、监控和日志都归档
   到同一个设备，不再与其他项目的设备页重复。
8. **WLOC 使用日志**：最近的坐标替换事件（时间、目标位置、来源 auto/manual）。

## 2. 架构总览

```
LuCI JS 前端 (view/wloc/location.js)
   │  fs.read(状态文件) / fetch(LuCI API)
   ▼
LuCI ucode 后端 (controller/wloc + model/wloc)
   │  exec /usr/sbin/wloc-ctl <cmd> ...
   ▼
wloc-ctl (Rust 小工具, 连 root-only UDS)
   │  FrameIo 控制帧
   ▼
wloc-service daemon (控制 API, 已实现 status/geo.set/geo.clear/enable/disable)
```

**三个数据面：**
- **状态**：`wloc-service` 定期写 `/var/run/wloc-service/status.json`
  （含阶段、模式、当前国家/城市、GPS、开关），前端 `fs.read` 轮询显示。
- **控制**：LuCI ucode 控制器调用 `/usr/sbin/wloc-ctl`（Rust 二进制，连 UDS 发
  控制帧并输出 JSON），前端通过 LuCI 的 ucode API 调用。
- **配置**：新增 `/etc/config/wloc-service`（UCI），持久化开关、模式、手动位置
  预设、证书链接；daemon 启动/重启时读取。

**为什么不直接开 HTTP 管理端口**：冻结的 `wloc.service/v1` 契约规定控制面只走
root-only UDS、无 TCP listener。`wloc-ctl` + UDS 保持该边界，LuCI 通过
ucode（以 root 运行）调用，权限由 LuCI ACL 控制。

## 3. 功能设计

### 3.1 证书获取（Safari）
- 页面"证书"卡片显示：证书指纹（SHA-256）、有效期、当前是否已签发。
- 按钮"获取证书"：展示描述文件链接（地址按路由器实际 LAN IP 动态生成，
  如 `http://192.168.31.1/wloc-ca.mobileconfig`，网关为 192.168.1.1 时自动
  变为 `http://192.168.1.1/wloc-ca.mobileconfig`），并可一键重新生成描述文件
  （调用 `wloc-ctl cert-export` 或在 LuCI 后端执行 `export-mobileconfig.sh`）。
- 提示步骤：Safari 打开链接 → 安装描述文件 → 证书信任设置开启完全信任。

### 3.2 定位模式切换（默认/手动）
- 单选："自动（跟随节点）" / "手动位置"。
- 切手动 → 进入手动预设编辑区；切自动 → 显示"跟随节点"提示。
- 持久化到 UCI `wloc-service.@main[0].geo_source`（`auto`/`manual`）+ 手动坐标；
  daemon 启动时按配置初始化 `geo_source`，运行中也可通过 `geo.set`/`geo.clear` 即时切。

### 3.3 WLOC 模块开关
- 开关 `wloc-service.enabled`（UCI，持久化）+ 运行时 `control.enable`/`disable`
  （现有状态机）。UI 显示服务阶段（disabled/starting/intercepting 等）。
- 关闭时：状态快照 `service_phase` 变 disabled，代理对 WLOC 响应 fail-open
  透传（已有行为），重定向规则保留但服务不拦截。

### 3.4 当前定位地理信息
- 只读卡片：国家码、城市（来自状态扩展字段 `geo.country_code`/`geo.city`，
  新增）、模式、观测时间。
- 数据来源：`wloc-service` 在 Geo 解析时记录国家/城市到状态文件（不含坐标）。

### 3.5 完整 GPS 数值
- 只读：`latitude` / `longitude` / `accuracy`（当前生效的 patch 目标）。
- **隐私边界**：坐标只进本地状态文件（root 可读），不进入控制 API 的公开
  响应（`status.get` 保持 coordinate-free）；由 LuCI（管理员会话）读取显示。
- 数据来源：`wloc-service` 写状态文件时附带当前 `PatchTarget` 坐标。

### 3.6 手动定位搜索与保存
- 输入框：搜索地名（如 "London, UK"）→ 按钮"搜索" → 调 `wloc-ctl geo-search`
  （后端连 UDS 发 `geo.set {query}`，Nominatim geocode）→ 显示结果坐标。
- 或直接输入 `latitude`/`longitude` 文本框。
- 按钮"保存并应用"：`geo.set` 应用 + UCI 持久化（`wloc-service.@main[0]`
  写 `manual_lat`/`manual_lon`），重启后仍生效。
- 保存的预设列表：可保存多个（`wloc-service.@preset[n]`），一键选择应用。

### 3.7 独立项目与设备档案
- **独立产品插件**：`luci-app-wificalling-location-gateway`（独立 IPK），菜单
  `admin/services/wificalling-location-gateway` 同时管理本项目内的 Gateway 与 WLOC。
- **页面结构**：
  ```
  admin/services/wificalling-location-gateway/
    ├── WCG Setting                 (Gateway 节点与设备设置)
    ├── WCG Status & Logs           (合并 Gateway 状态与活动日志)
    ├── WLOC Setting                (合并 Overview 与 Basic Settings)
    ├── WLOC Devices                (每台设备的节点/定位/启用/状态/日志)
    ├── WLOC Status & Logs
    ├── WCG WLOC Service Monitor    (统一服务监控)
    ├── Component Update
    └── Help
  ```
- **共享层**：本项目自己的 Gateway 节点、WLOC 节点引用和设备档案共享 sing-box
  provider；Gateway 与 WLOC 各自保留数据面，但由统一 supervisor、统一更新/回滚
  和统一监控协调。
- **设备语义**：`fixed` 是该设备档案明确绑定的 WLOC 节点；`auto` 跟随这个
  节点出口；`manual` 把坐标保存到这个设备档案。不存在没有目标对象的
  “Follow gateway” 或全局手动定位。
- **组件更新**：独立页面负责上传前置的本地包、设备/固件/架构/包格式检查、签名
  校验、空间检查、应用和回滚；健康页只显示运行状态。

### 3.8 WLOC 使用日志
- 新日志：`wloc-service` 追加事件到 `/var/run/wloc-service/events.jsonl`
  （时间、目标国家/城市、坐标、模式、来源 auto/manual、替换前后字节数）。
- 前端"WLOC 日志"卡片轮询 `fs.read` 显示最近 N 条。
- 不记录原始 WLOC 响应内容（隐私），只有替换元数据。

## 4. 控制 API 扩展需求（wloc-service）

现有 `status.get`/`geo.set`/`geo.clear`/`control.*` 已覆盖大部分；为 UI 补充：

| API | 用途 | 说明 |
|---|---|---|
| `status.get` 扩展 `geo.country_code`/`geo.city` | 地理信息 | 不含坐标 |
| `status.json` 文件（新） | 前端读取 | 含 GPS（root 文件） |
| `geo.set`（已有，query/坐标） | 手动搜索/应用 | 需要 UCI 持久化配合 |
| `events.jsonl`（新） | 使用日志 | 替换元数据 |
| `control.enable/disable`（已有） | 模块开关 | |

持久化：`geo.set` 应用后写 UCI（由 LuCI 后端做，daemon 不写 UCI）；daemon
启动时读 UCI 初始化 `geo_source`/手动预设。

## 5. 配置映射（UCI `/etc/config/wloc-service`）

```
config wloc-service 'main'
    option enabled '1'          # 模块开关
    option geo_source 'auto'    # auto | manual
    option manual_lat '51.5074'
    option manual_lon '-0.1278'
  option node_ref 'wloc_node_1'
    option probe_port '18080'
    option ca_path '/etc/wloc-service/ca.pem'
    option ca_key '/etc/wloc-service/ca.key'

config preset 'hong_kong'       # 手动预设（可多个）
    option label '香港'
    option latitude '22.3193'
    option longitude '114.1694'
```

## 6. 文件清单（实现计划）

```
openwrt/files/etc/config/wloc-service              # UCI 配置（含 geo_source/manual/presets）
openwrt/files/usr/sbin/wloc-ctl                    # 控制 CLI (Rust)
openwrt/files/usr/sbin/export-mobileconfig.sh      # Safari 证书描述文件生成
openwrt/Makefile                                   # wloc-service + wloc-ctl 包
openwrt/luci-app-wificalling-location-gateway/     # 独立 LuCI 插件包
  Makefile                                         # luci.mk 打包
  files/usr/share/luci/menu.d/luci-app-wificalling-location-gateway.json  # 菜单(两模块并列)
  files/usr/share/rpcd/acl.d/luci-app-wificalling-location-gateway.json   # ACL
  files/usr/share/rpcd/ucode/luci.wloc.uc          # RPC 后端（白名单调 wloc-ctl）
  files/www/luci-static/resources/view/wificalling-location-gateway/wloc.js  # WLOC 页面(8 功能)
  files/usr/lib/lua/luci/i18n/wificalling-location-gateway.zh-cn.po  # 中文
src/config/uci.rs                                  # daemon 启动读 UCI
src/bin/wloc-ctl.rs                                # CLI 源码
docs/ui/INTEGRATION_UI.md                          # 本文档
```

## 7. 安全边界

- 控制面仍走 root-only UDS；`wloc-ctl` 必须 root（procd 拥有）执行。
- LuCI 通过 ucode 以 root 运行 → 由 LuCI ACL（`luci-app-wloc`）限制访问，
  普通用户不可见。
- `status.json` 含 GPS → 文件权限 0640 root:root；LuCI 管理员才可读。
- 控制 API 的 `status.get` 保持 coordinate-free（隐私不回归）。
- 日志只记录替换元数据，不记录原始 WLOC 响应/设备标识/精确用户位置。

## 8. 实现顺序

1. wloc-service 状态文件 + `geo` 国家/城市扩展 + `events.jsonl`（后端数据面）。
2. `wloc-ctl` CLI（连 UDS，供 LuCI 后端调用）。
3. LuCI ucode 后端 + 菜单 + 页面（8 项功能）。
4. UCI 配置 + daemon 启动初始化。
5. 真机联调（证书/切换/搜索保存/日志）。
