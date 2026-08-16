'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';

// FAQ：Wi-Fi Calling 与 WLOC 定位的使用步骤和注意事项（中英双语）。

// The CA profile is served by this router's uhttpd; derive the address
// from the page the admin is using instead of a hardcoded subnet.
var profileHost = (location.hostname.indexOf(':') >= 0)
	? '[' + location.hostname + ']' : location.hostname;
var PROFILE_URL = 'http://' + profileHost + '/wloc-ca.mobileconfig';

var CONTENT = {
	zh: {
		title: '使用帮助（FAQ）',
		intro: '以下是 Wi-Fi Calling 与 WLOC 定位的使用步骤和常见注意事项。',
		wfcStepsTitle: 'Wi-Fi 通话 · 使用步骤',
		wfcSteps: [
			'1. 进入「Wi-Fi 通话设置」页面，在「导入代理节点」中粘贴节点链接（支持 AnyTLS、Hysteria2/Hy2、TUIC、VLESS、VMess、Trojan、WireGuard(wg://)），或点击「添加代理节点」手动填写节点信息。',
			'2. 点击「保存并应用」，节点即生效。',
			'3. 在「设备策略」中点击「添加局域网设备」，填写设备名称、路由模式（独立隧道或跟随网关）、绑定节点与局域网 IP。',
			'4. 再次「保存并应用」。',
			'5. 在 iPhone 上连接本路由器 Wi-Fi，并开启「设置 → 蜂窝网络 → Wi-Fi 通话」。',
			'6. 到「Wi-Fi 通话监控与日志」页查看隧道状态：显示「已注册」表示观察到 ASSURED 双向 UDP 4500 隧道，Wi-Fi 通话通道已建立。'
		],
		wfcNotesTitle: 'Wi-Fi 通话 · 注意事项',
		wfcNotes: [
			'「已注册」是网络层证据，不代表运营商已激活 Wi-Fi 通话服务，请以实际拨号/通话为准。',
			'节点导入链接仅在本浏览器中本地解析，不会发送到任何外部服务。',
			'设备策略中的节点需先保存节点后，刷新页面才能选择。',
			'活动日志只记录握手结果与持续加密流量等元数据；隧道内容全程加密，通话/短信无法区分，号码与消息内容不可见。',
			'若手机显示「未检测到」，请检查节点连通性、设备策略绑定与 iPhone 的 Wi-Fi 通话开关。',
			'旁路由（自身不提供 DHCP）同样可用：设备是否在线通过 ARP 邻居表识别，无需 DHCP 租约；请把设备的网关指向旁路由 IP。'
		],
		wlocStepsTitle: 'WLOC 定位 · 使用步骤',
		wlocSteps: [
			'1. 安装根证书：在 iPhone 的 Safari 中打开 {profile-url}，安装配置描述文件。',
			'2. 开启完全信任：iPhone「设置 → 通用 → 关于本机 → 证书信任设置」，将 wloc-service 根证书开启完全信任。',
			'3. 在「WLOC 设置」页面打开「启用 WLOC 拦截」。',
			'4. 选择定位模式：自动（跟随节点）——跟随测试设备绑定的节点出口定位；手动位置——用「手动搜索」搜索地名（如 Tokyo）或直接输入坐标，也可从「已保存的位置」一键应用。',
			'5. 切换模式或更换位置后，在 iPhone 上重新触发定位（开关一次飞行模式，或关闭再打开 Wi-Fi，或重新打开地图/天气应用）。',
			'6. 到「WLOC 监控与日志」页确认当前定位、GPS 坐标与「定位目标更新」事件。'
		],
		wlocNotesTitle: 'WLOC 定位 · 注意事项',
		wlocNotes: [
			'iPhone 必须关闭 Cloudflare WARP（或其他 VPN 类应用），否则会绕过路由器重定向，定位替换不生效。',
			'证书未信任时 HTTPS 拦截会失败：请确认描述文件已安装且「完全信任」已开启。',
			'自动模式跟随节点出口 IP 的地理位置；手动模式的 GPS 只保存在路由器本地，不会外传。',
			'定位请求由 iPhone 的应用（地图、天气等）触发；若地图未刷新，请重新触发定位或稍等片刻。',
			'WLOC 使用日志最多显示最近 20 条，可一键清空；日志仅记录替换元数据，不记录原始 WLOC 响应内容。',
			'配置（开关、模式、手动坐标、预设）保存在 /etc/config/wloc-service，重启后仍然生效。',
			'「服务状态」页（服务 → 服务状态）集中显示 wloc-service 与 Wi-Fi 通话网关的进程、配置、规则、补丁与节点健康，每 10 秒自动刷新；异常项以红点标出。',
			'每个代理节点行的「nodeTest」按钮可随时执行一次新的连接测试：WireGuard 节点进行真实握手并显示出口 IP，其他协议执行 TCP 连通性探测；若握手失败，状态列会显示简短原因（超时 / 不可达 / 配置缺失），鼠标悬停可查看完整解释。'
		]
	},
	en: {
		title: 'Help (FAQ)',
		intro: 'Step-by-step usage and notes for Wi-Fi Calling and WLOC location spoofing.',
		wfcStepsTitle: 'Wi-Fi Calling · Usage steps',
		wfcSteps: [
			'1. Open the "Wi-Fi Calling Settings" page and paste a node link under "Import proxy node" (AnyTLS, Hysteria2/Hy2, TUIC, VLESS, VMess, Trojan, WireGuard (wg://)), or click "Add proxy node" to enter node details manually.',
			'2. Click "Save & Apply" to activate the node.',
			'3. Under "Device Policies", click "Add LAN device" and enter the device name, routing mode (independent tunnel or follow gateway), the bound node and the LAN IP.',
			'4. "Save & Apply" again.',
			'5. On the iPhone, join this router\'s Wi-Fi and enable Wi-Fi Calling (Settings > Cellular > Wi-Fi Calling).',
			'6. Check the "Wi-Fi Calling Monitor & Log" page: "Registered" means an ASSURED bidirectional UDP 4500 tunnel was observed and the Wi-Fi Calling channel is up.'
		],
		wfcNotesTitle: 'Wi-Fi Calling · Notes',
		wfcNotes: [
			'"Registered" is network-level evidence, not carrier activation confirmation - verify by actually placing a call.',
			'Node import links are parsed locally in this browser and are never sent to any external service.',
			'Save a node first, then reload the page, before you can select it for a device policy.',
			'The activity log only records handshake results and sustained encrypted traffic. Tunnel content is fully encrypted: calls are inferred from sustained bidirectional traffic, SMS cannot be distinguished, and phone numbers or message content are never visible.',
			'If the phone shows "Not detected", check node connectivity, the device policy binding and the iPhone Wi-Fi Calling switch.',
			'A secondary router without DHCP works too: device liveness is detected via the ARP cache, so no DHCP lease is required; just point the device gateway to the secondary router\'s IP.'
		],
		wlocStepsTitle: 'WLOC Location · Usage steps',
		wlocSteps: [
			'1. Install the root CA: open {profile-url} in Safari on the iPhone and install the configuration profile.',
			'2. Enable full trust: on the iPhone go to Settings > General > About > Certificate Trust Settings and enable full trust for the wloc-service root CA.',
			'3. Turn on "Enable WLOC interception" on the WLOC Settings page.',
			'4. Choose a location mode: Auto (follow node) follows the bound node\'s exit; Manual lets you search a place name (e.g. Tokyo) or enter coordinates, or apply a saved preset in one click.',
			'5. After switching the mode or location, re-trigger location on the iPhone (toggle airplane mode once, or toggle Wi-Fi, or reopen the Maps/Weather app).',
			'6. Confirm the current location, GPS coordinates and "target updated" events on the WLOC Monitor & Log page.'
		],
		wlocNotesTitle: 'WLOC Location · Notes',
		wlocNotes: [
			'Cloudflare WARP (or any VPN app) must be OFF on the iPhone, otherwise the router redirect is bypassed and spoofing will not take effect.',
			'If the CA is not trusted, HTTPS interception fails: make sure the profile is installed and full trust is enabled.',
			'Auto mode follows the location of the node exit IP; manual GPS stays local to the router and is never sent out.',
			'Location requests are triggered by iPhone apps (Maps, Weather, etc.). If maps do not refresh, re-trigger location and wait a moment.',
			'The WLOC usage log keeps the newest 20 entries and can be cleared with one click; it only records replacement metadata, never raw WLOC responses.',
			'Settings (switch, mode, manual coordinates, presets) are stored in /etc/config/wloc-service and survive reboots.',
			'"Service Status" (Services > Service Status) shows both services in one place: daemon processes, config validity, nftables rules, build patches and node health, refreshed every 10 seconds; anything unhealthy is marked with a red dot.',
			'The "nodeTest" button on every proxy node row runs a fresh connection test on demand: WireGuard nodes perform a real handshake and show the verified exit IP, other protocols get a TCP reachability probe; when a handshake fails, the status cell shows a short reason (Timeout / Unreachable / Missing config) and hovering reveals the full explanation.'
		]
	}
};

function stepsCard(title, steps) {
	return E('div', { 'class': 'cbi-section' }, [
		E('h3', {}, title),
		E('ol', {}, steps.map(function(s) { return E('li', {}, s); }))
	]);
}

// Replace the {profile-url} placeholder with a tappable link to the CA
// profile (the plain text is used when the URL cannot be derived).
function withProfileUrl(item) {
	var parts = String(item).split('{profile-url}');
	if (parts.length === 1)
		return item;
	return [parts[0], E('a', { href: PROFILE_URL, target: '_blank' }, PROFILE_URL), parts[1]];
}

function notesCard(title, notes) {
	return E('div', { 'class': 'cbi-section' }, [
		E('h3', {}, title),
		E('ul', {}, notes.map(function(s) { return E('li', {}, s); }))
	]);
}

return view.extend({
	render: function() {
		// The tab menu is re-rendered in English on every page switch; like
		// the other views, localize it here so it stays in the UI language.
		wlocI18n.localizeTabs();
		// Follow the LuCI interface language automatically (the body class
		// is `lang_en` on this firmware, `lang-en` on others).
		var cls = document.body.className;
		var lang = (cls.indexOf('lang-en') >= 0 || cls.indexOf('lang_en') >= 0) ? 'en' : 'zh';
		var c = CONTENT[lang];
		return E([], [
			E('h2', {}, c.title),
			E('p', {}, c.intro),
			stepsCard(c.wfcStepsTitle, c.wfcSteps),
			notesCard(c.wfcNotesTitle, c.wfcNotes),
			stepsCard(c.wlocStepsTitle, c.wlocSteps.map(withProfileUrl)),
			notesCard(c.wlocNotesTitle, c.wlocNotes)
		]);
	}
});
