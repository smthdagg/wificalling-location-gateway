'use strict';
'require baseclass';

// 轻量 i18n：页面文案以英文为原文，中文界面通过这张映射表替换。
// LuCI 26 本机无法编译 .lmo 翻译包，因此由前端按界面语言直接切换。

var ZH = {
	/* ---- tabs ---- */
	'Wi-Fi Calling Settings': 'Wi-Fi 通话设置',
	'Wi-Fi Calling Monitor & Log': 'Wi-Fi 通话监控与日志',
	'WLOC Settings': 'WLOC 设置',
	'WLOC Monitor & Log': 'WLOC 监控与日志',
	'Help (FAQ)': '使用帮助（FAQ）',

	/* ---- wloc settings ---- */
	'WLOC location interception: spoofs the Apple WLOC response so the test device reports the gateway-chosen location. GPS values stay on this router.':
		'WLOC 定位拦截：改写 Apple WLOC 响应，使测试设备上报网关指定的位置。GPS 数值只保留在本路由器上。',
	'Module': '模块',
	'Enable WLOC interception': '启用 WLOC 拦截',
	'Turns the WLOC rewrite on/off. The nftables redirect stays in place; while off, Apple WLOC traffic passes through untouched.':
		'打开/关闭 WLOC 改写。nftables 重定向规则保留；关闭时 Apple WLOC 流量原样透传。',
	'Service phase': '服务阶段',
	'Location mode': '定位模式',
	'Auto follows the sing-box node bound to the test device. Manual uses the search result or coordinates below.':
		'自动模式跟随测试设备绑定的 sing-box 节点出口；手动模式使用下方搜索或输入的坐标。',
	'Auto (follow node)': '自动（跟随节点）',
	'Manual location': '手动位置',
	'Manual latitude': '手动纬度',
	'Manual longitude': '手动经度',
	'Manual search': '手动搜索',
	'Place name': '地名',
	'(online search)': '（在线搜索）',
	'e.g. London, UK': '例如：伦敦',
	'Search': '搜索',
	'Search result: ': '搜索结果：',
	' (lat ': '（纬度 ',
	', lon ': '，经度 ',
	') - click "Apply coordinates" to activate.': '），确认后点击「应用坐标」。',
	'Or enter coordinates': '或直接输入坐标',
	'Apply coordinates': '应用坐标',
	'Saved locations': '已保存的位置',
	'Label': '名称',
	'Latitude': '纬度',
	'Longitude': '经度',
	'Apply': '应用',
	'Delete': '删除',
	'Add saved location': '添加已保存位置',
	'No saved locations yet.': '暂无已保存的位置。',
	'Preset saved.': '预设已保存。',
	'Certificate (Safari install)': '证书（Safari 安装）',
	'Profile link': '描述文件链接',
	'Install steps': '安装步骤',
	'1. Open the profile link in Safari on the test iPhone. ': '1. 在测试 iPhone 的 Safari 中打开描述文件链接。 ',
	'2. Install the configuration profile. ': '2. 安装配置描述文件。 ',
	'3. Enable full trust in Settings > General > About > Certificate Trust Settings.':
		'3. 在 设置 > 通用 > 关于本机 > 证书信任设置 中开启完全信任。',
	'Regenerate profile': '重新生成描述文件',
	'Applied': '已应用',
	'Search result is now the active location. Verify on the iPhone.': '搜索结果已成为当前生效位置。请在 iPhone 上验证。',
	'Coordinates are now the active location.': '坐标已成为当前生效位置。',
	'Search failed': '搜索失败',
	'Place not found': '未找到该地名',
	'Apply failed': '应用失败',
	'Switch failed': '切换失败',
	'Mode switch failed': '模式切换失败',
	'Enter and apply manual coordinates first.': '请先输入并应用手动坐标。',
	'Regenerate failed': '重新生成失败',
	'Profile ready': '描述文件已就绪',
	'On the iPhone open Safari and visit %s, then enable full trust in Settings > General > About > Certificate Trust Settings.':
		'在 iPhone 上用 Safari 打开 %s，然后在 设置 > 通用 > 关于本机 > 证书信任设置 中开启完全信任。',
	'Preset has no coordinates.': '预设没有坐标。',
	'Preset is now the active location.': '预设已成为当前生效位置。',
	'Save failed': '保存失败',
	'Cancel': '取消',
	'Save': '保存',
	'Follow device': '定位跟随设备',
	'The device whose bound node the WLOC location follows (its exit IP drives auto mode).':
		'WLOC 定位跟随的设备：自动模式按其绑定节点的出口 IP 定位。',
	'Device saved. WLOC now follows its node.': '已保存。WLOC 定位将跟随该设备的节点。',
	'CA info': '证书信息',
	'Fingerprint': '证书指纹',
	'Issued at': '签发时间',
	'Expires at': '到期时间',
	'Unknown': '未知',
	'New CA generated. Reinstall and trust it on the iPhone.': '新证书已生成。请在 iPhone 上重新安装描述文件并开启完全信任。',
	'CA info missing - start the service first.': '证书信息不可用，请先启动 wloc-service。',
	'Certificate trust': '证书信任状态',
	'Last handshake': '最近握手',
	'Handshake failed - the iPhone does not trust this CA. Reinstall the profile and enable full trust.': '握手失败——iPhone 未信任当前证书。请重新安装描述文件并开启完全信任。',
	'Handshake ok': '握手正常',
	'Verify iPhone certificate': '校验 iPhone 证书',
	'Paste the fingerprint shown on the iPhone (Settings > General > VPN & Device Management > wloc-service profile).':
		'粘贴 iPhone 上显示的证书指纹（设置 > 通用 > VPN与设备管理 > wloc-service 描述文件）。',
	'Verify': '校验',
	'Match - the iPhone trusts this CA.': '匹配——iPhone 信任当前证书。',
	'Mismatch - reinstall the profile on the iPhone and enable full trust.': '不匹配——请在 iPhone 上重新安装描述文件并开启完全信任。',
	'Gateway fingerprint: ': '网关证书指纹：',
	'No handshakes yet.': '暂无握手记录。',
	'Generate a new root certificate?': '生成新的根证书？',
	'This replaces the root CA. All devices must reinstall the profile and enable full trust again.':
		'这将更换根证书。所有设备都需要重新安装描述文件并再次开启完全信任。',
	'Generate new CA': '生成新证书',

	/* ---- wloc monitor ---- */
	'Current location': '当前定位',
	'Shows the effective location target: auto follows the node exit, manual uses the coordinates from the settings page. GPS values stay on this router.':
		'显示当前生效的定位目标：自动模式跟随节点出口，手动模式使用设置页的坐标。GPS 数值只保留在本路由器上。',
	'Manual': '手动',
	'Auto': '自动',
	'Country': '国家/地区',
	'City': '城市',
	'Timezone': '时区',
	'GPS (lat / lon)': 'GPS 坐标（纬度 / 经度）',
	'Geo state': '定位状态',
	'Observed at': '观测时间',
	'Exit IP': '出口 IP',
	'WLOC usage log': 'WLOC 使用日志',
	'Records each location target update (time, place, source auto/manual). Raw WLOC responses are never recorded.':
		'记录每次定位目标更新（时间、目标位置、来源 自动/手动）。不记录原始 WLOC 响应内容。',
	'Records: ': '记录数： ',
	'Clear log': '清空日志',
	'Clear WLOC usage log?': '清空 WLOC 使用日志？',
	'This clears the local history of WLOC location events. Location interception settings are not affected.':
		'此操作将清空 WLOC 定位事件的本地历史记录。定位拦截设置不受影响。',
	'WLOC usage log cleared.': 'WLOC 使用日志已清空。',
	'Unable to clear log: ': '无法清空日志： ',
	'Time': '时间',
	'Event': '事件',
	'Location': '位置',
	'Source': '来源',
	'Target updated': '定位目标更新',
	'WLOC response rewritten': 'WLOC 响应已重写',

	/* ---- wfc monitor ---- */
	'Device tunnel status': '设备隧道状态',
	'Registered means an ASSURED bidirectional UDP 4500 tunnel was observed. This is network evidence, not carrier activation confirmation.':
		'已注册表示观察到 ASSURED 双向 UDP 4500 隧道。这是网络证据，不代表运营商已激活服务。',
	'Registered': '已注册',
	'Connecting': '连接中',
	'Not detected': '未检测到',
	'Likely registered': '疑似已注册',
	'Active traffic': '活动流量',
	'NAT-T seen': '已见 NAT-T',
	'Negotiating': '协商中',
	'No session': '无会话',
	'Yes': '是',
	'No': '否',
	'Device': '设备',
	'IP': 'IP',
	'Wi-Fi Calling status': 'Wi-Fi 通话状态',
	'Node': '节点',
	'ePDG IP': 'ePDG IP',
	'UDP 500/4500': 'UDP 500/4500',
	'ASSURED': 'ASSURED',
	'Packets': '数据包',
	'Last activity': '最后活动',
	'Encrypted IMS activity log': '加密 IMS 活动日志',
	'Records handshake success or failure and sustained encrypted communication such as ringing or calls. Brief traffic bursts are not logged. The tunnel content is encrypted: a call is inferred from sustained bidirectional traffic, SMS cannot be distinguished, and phone numbers or message content are never visible.':
		'记录握手成功或失败，以及响铃、通话等持续加密通讯。短暂流量脉冲不记录。隧道内容全程加密：通话根据持续双向流量推断，短信无法区分，电话号码与消息内容永远不可见。',
	'Handshake success': '握手成功',
	'Handshake failed': '握手失败',
	'Sustained traffic': '持续流量',
	'Call in progress (inferred from sustained encrypted traffic)': '通话进行中（由持续加密流量推断）',
	'Encrypted activity; call/SMS unknown': '加密活动；通话/短信无法区分',
	'Wi-Fi Calling': 'Wi-Fi 通话',
	'Activity': '活动',
	'Packet delta': '数据包增量',
	'Meaning': '含义',
	'Clear activity log?': '清空活动日志？',
	'This permanently removes only the Wi-Fi Calling activity history. Settings and system logs are not affected.':
		'此操作仅永久删除 Wi-Fi Calling 活动历史。设置与系统日志不受影响。',
	'Activity log cleared.': '活动日志已清空。',
	'Activity log recording is disabled. Enable it in Settings.': '活动日志记录已禁用。请在设置中启用。'
};

return baseclass.extend({
	isEn: function() {
		var cls = document.body.className;
		return cls.indexOf('lang-en') >= 0 || cls.indexOf('lang_en') >= 0;
	},

	// Translate an English base string into the current UI language.
	t: function(text) {
		if (this.isEn())
			return text;
		return ZH[text] || text;
	},

	// Replace the top tab labels with the current UI language.
	localizeTabs: function() {
		var self = this;
		var apply = function() {
			document.querySelectorAll('#tabmenu a, .tabs a').forEach(function(a) {
				var mapped = self.t(a.textContent.trim());
				if (mapped !== a.textContent)
					a.textContent = mapped;
			});
		};
		apply();
		// LuCI replaces the tab bar after a view's render() returns. Repeat
		// after the next two paints so the final DOM, including FAQ, keeps
		// the selected interface language.
		if (typeof window !== 'undefined' && typeof window.requestAnimationFrame === 'function') {
			window.requestAnimationFrame(function() {
				apply();
				window.requestAnimationFrame(apply);
			});
		}
	}
});
