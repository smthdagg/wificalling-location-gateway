'use strict';
'require view';

// FAQ：Wi-Fi Calling 与 WLOC 定位的使用步骤和注意事项。

function stepsCard(title, steps) {
	return E('div', { 'class': 'cbi-section' }, [
		E('h3', {}, title),
		E('ol', {}, steps.map(function(s) { return E('li', {}, s); }))
	]);
}

function notesCard(title, notes) {
	return E('div', { 'class': 'cbi-section' }, [
		E('h3', {}, title),
		E('ul', {}, notes.map(function(s) { return E('li', {}, s); }))
	]);
}

return view.extend({
	render: function() {
		return E([], [
			E('h2', {}, _('使用帮助（FAQ）')),
			E('p', {}, _('以下是 Wi-Fi Calling 与 WLOC 定位的使用步骤和常见注意事项。'))

			/* ---------- Wi-Fi Calling ---------- */
			, stepsCard(_('Wi-Fi 通话 · 使用步骤'), [
				_('1. 进入「Wi-Fi 通话设置」页面，在「导入代理节点」中粘贴节点链接（支持 AnyTLS、Hysteria2/Hy2、TUIC、VLESS、VMess、Trojan、WireGuard(wg://)），或点击「添加代理节点」手动填写节点信息。'),
				_('2. 点击「保存并应用」，节点即生效。'),
				_('3. 在「设备策略」中点击「添加局域网设备」，填写设备名称、路由模式（独立隧道或跟随网关）、绑定节点与局域网 IP。'),
				_('4. 再次「保存并应用」。'),
				_('5. 在 iPhone 上连接本路由器 Wi-Fi，并开启「设置 → 蜂窝网络 → Wi-Fi 通话」。'),
				_('6. 到「Wi-Fi 通话监控与日志」页查看隧道状态：显示「已注册」表示观察到 ASSURED 双向 UDP 4500 隧道，Wi-Fi 通话通道已建立。')
			])
			, notesCard(_('Wi-Fi 通话 · 注意事项'), [
				_('「已注册」是网络层证据，不代表运营商已激活 Wi-Fi 通话服务，请以实际拨号/通话为准。'),
				_('节点导入链接仅在本浏览器中本地解析，不会发送到任何外部服务。'),
				_('设备策略中的节点需先保存节点后，刷新页面才能选择。'),
				_('活动日志只记录握手结果与持续加密流量等元数据；隧道内容全程加密，通话/短信无法区分，号码与消息内容不可见。'),
				_('若手机显示「未检测到」，请检查节点连通性、设备策略绑定与 iPhone 的 Wi-Fi 通话开关。')
			])

			/* ---------- WLOC ---------- */
			, stepsCard(_('WLOC 定位 · 使用步骤'), [
				_('1. 安装根证书：在 iPhone 的 Safari 中打开 http://192.168.31.1/wloc-ca.mobileconfig，安装配置描述文件。'),
				_('2. 开启完全信任：iPhone「设置 → 通用 → 关于本机 → 证书信任设置」，将 wloc-service 根证书开启完全信任。'),
				_('3. 在「WLOC 设置」页面打开「启用 WLOC 拦截」。'),
				_('4. 选择定位模式：自动（跟随节点）——跟随测试设备绑定的节点出口定位；手动位置——用「手动搜索」搜索地名（如 Tokyo）或直接输入坐标，也可从「已保存的位置」一键应用。'),
				_('5. 切换模式或更换位置后，在 iPhone 上重新触发定位（开关一次飞行模式，或关闭再打开 Wi-Fi，或重新打开地图/天气应用）。'),
				_('6. 到「WLOC 监控与日志」页确认当前定位、GPS 坐标与「定位目标更新」事件。')
			])
			, notesCard(_('WLOC 定位 · 注意事项'), [
				_('iPhone 必须关闭 Cloudflare WARP（或其他 VPN 类应用），否则会绕过路由器重定向，定位替换不生效。'),
				_('证书未信任时 HTTPS 拦截会失败：请确认描述文件已安装且「完全信任」已开启。'),
				_('自动模式跟随节点出口 IP 的地理位置；手动模式的 GPS 只保存在路由器本地，不会外传。'),
				_('定位请求由 iPhone 的应用（地图、天气等）触发；若地图未刷新，请重新触发定位或稍等片刻。'),
				_('WLOC 使用日志最多显示最近 20 条，可一键清空；日志仅记录替换元数据，不记录原始 WLOC 响应内容。'),
				_('配置（开关、模式、手动坐标、预设）保存在 /etc/config/wloc-service，重启后仍然生效。')
			])
		]);
	}
});
