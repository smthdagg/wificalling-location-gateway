'use strict';
'require view';
'require fs';
'require poll';
'require dom';
'require ui';
'require uci';

// Wi-Fi Calling 监控与日志（合并页）：设备隧道状态 + 加密 IMS 活动日志。

return view.extend({
	load: function() {
		return Promise.all([
			L.resolveDefault(fs.read('/var/run/wificalling-gateway/status.json'), '{}'),
			L.resolveDefault(fs.read('/var/run/wificalling-gateway/events.log'), ''),
			uci.load('wificalling-gateway')
		]);
	},

	render: function(data) {
		var raw = data[0];
		var eventsRaw = data[1];
		var logEnabled = uci.get('wificalling-gateway', 'main', 'log_enabled');

		function when(epoch) { return epoch ? new Date(epoch * 1000).toLocaleString() : '-'; }
		function parse(value) { try { return JSON.parse(value); } catch (e) { return { devices: [] }; } }
		function wfcLabel(v) {
			switch (v) {
				case 'registered': return _('已注册');
				case 'connecting': return _('连接中');
				case 'not_detected': return _('未检测到');
				case 'likely_registered': return _('疑似已注册');
				case 'active_traffic': return _('活动流量');
				case 'nat_t_seen': return _('已见 NAT-T');
				case 'negotiating': return _('协商中');
				case 'no_session': return _('无会话');
				default: return v || '-';
			}
		}
		function activityLabel(v) {
			switch (v) {
				case 'handshake_success': return _('握手成功');
				case 'handshake_failed': return _('握手失败');
				case 'sustained_traffic': return _('持续流量');
				default: return v || '-';
			}
		}
		function meaningLabel(v) {
			switch (v) {
				case 'likely_call': return _('通话进行中（由持续加密流量推断）');
				default: return _('加密活动；通话/短信无法区分');
			}
		}
		function lines(value) { return value.trim() ? value.trim().split('\n').reverse() : []; }

		/* ---------- 设备隧道状态 ---------- */
		var statusBody = E('tbody', {}, []);
		function statusRows(source) {
			return (source.devices || []).map(function(d) {
				var values = [d.label, d.ip, wfcLabel(d.wificalling || d.state), d.node || '-', d.epdg_ip || '-',
					(d.ike_seen ? '500' : '-') + ' / ' + (d.nat_t_seen ? '4500' : '-'),
					d.assured ? _('是') : _('否'), d.sent_packets + ' ↑ / ' + d.reply_packets + ' ↓', when(d.last_activity)];
				return E('tr', { class: 'tr' }, values.map(function(x) { return E('td', { class: 'td' }, String(x)); }));
			});
		}
		function renderStatus(value) { dom.content(statusBody, statusRows(parse(value))); }
		renderStatus(raw);

		/* ---------- 活动日志 ---------- */
		var logBody = E('tbody', {}, []);
		function logRows(value) {
			return lines(value).map(function(line) {
				var f = line.split('|');
				return E('tr', { class: 'tr' }, [when(Number(f[0])), f[1], f[2], wfcLabel(f[7]), activityLabel(f[3]), (f[4] || '0') + ' ↑ / ' + (f[5] || '0') + ' ↓', meaningLabel(f[6])].map(function(x) { return E('td', { class: 'td' }, String(x)); }));
			});
		}
		var logCount = E('span', {}, String(lines(eventsRaw).length));
		function renderLog(value) { dom.content(logBody, logRows(value)); dom.content(logCount, String(lines(value).length)); }
		renderLog(eventsRaw);

		var clear = E('button', { class: 'btn cbi-button-negative', click: function() {
			ui.showModal(_('清空活动日志？'), [E('p', {}, _('此操作仅永久删除 Wi-Fi Calling 活动历史。设置与系统日志不受影响。')),
				E('div', { class: 'right' }, [E('button', { class: 'btn', click: ui.hideModal }, _('取消')),
				E('button', { class: 'btn cbi-button-negative', click: function() {
					fs.write('/var/run/wificalling-gateway/events.log', '').then(function() { renderLog(''); ui.hideModal(); ui.addNotification(null, E('p', {}, _('活动日志已清空。')), 'info'); }).catch(function(err) { ui.addNotification(null, E('p', {}, _('无法清空日志：') + ' ' + err.message), 'error'); });
				} }, _('清空日志'))])]);
		} }, _('清空日志'));

		/* ---------- 轮询刷新 ---------- */
		poll.add(function() {
			return L.resolveDefault(fs.read('/var/run/wificalling-gateway/status.json'), '{}').then(renderStatus);
		}, 5);
		poll.add(function() {
			return L.resolveDefault(fs.read('/var/run/wificalling-gateway/events.log'), '').then(renderLog);
		}, 5);

		var children = [
			E('h2', {}, _('Wi-Fi 通话监控与日志')),
			E('div', { class: 'cbi-section' }, [
				E('h3', {}, _('设备隧道状态')),
				E('p', {}, _('已注册表示观察到 ASSURED 双向 UDP 4500 隧道。这是网络证据，不代表运营商已激活服务。')),
				E('table', { class: 'table' }, [
					E('tr', { class: 'tr table-titles' }, [_('设备'), _('IP'), _('Wi-Fi 通话状态'), _('节点'), _('ePDG IP'), _('UDP 500/4500'), _('ASSURED'), _('数据包'), _('最后活动')].map(function(x) { return E('th', { class: 'th' }, x); })),
					statusBody
				])
			]),
			E('div', { class: 'cbi-section' }, [
				E('h3', {}, _('加密 IMS 活动日志')),
				E('p', {}, _('记录握手成功或失败，以及响铃、通话等持续加密通讯。短暂流量脉冲不记录。隧道内容全程加密：通话根据持续双向流量推断，短信无法区分，电话号码与消息内容永远不可见。')),
				E('p', {}, [_('记录数：') + ' ', logCount, ' ', clear]),
				E('table', { class: 'table' }, [
					E('tr', { class: 'tr table-titles' }, [_('时间'), _('设备'), _('IP'), _('Wi-Fi 通话'), _('活动'), _('数据包增量'), _('含义')].map(function(x) { return E('th', { class: 'th' }, x); })),
					logBody
				])
			])
		];
		if (logEnabled === '0')
			children.splice(3, 0, E('div', { class: 'alert-message warning' }, _('活动日志记录已禁用。请在设置中启用。')));
		return E([], children);
	}
});
