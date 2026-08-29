'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require rpc';
'require fs';
'require poll';
'require dom';
'require ui';
'require uci';

// Wi-Fi Calling 监控与日志（合并页）：设备隧道状态 + 加密 IMS 活动日志。

var clearLog = rpc.declare({
	object: 'luci.wloc',
	method: 'clear_log',
	params: [ 'log' ]
});

return view.extend({
	load: function() {
		return Promise.all([
			L.resolveDefault(fs.read('/var/run/wificalling-gateway/status.json'), '{}'),
			L.resolveDefault(fs.read('/var/run/wificalling-gateway/events.log'), ''),
			uci.load('wificalling-gateway')
		]);
	},

	render: function(data) {
		wlocI18n.localizeTabs();
		var raw = data[0];
		var eventsRaw = data[1];
		var logEnabled = uci.get('wificalling-gateway', 'main', 'log_enabled');

		function when(epoch) { return epoch ? new Date(epoch * 1000).toLocaleString() : '-'; }
		function parse(value) { try { return JSON.parse(value); } catch (e) { return { devices: [] }; } }
		function wfcLabel(v) {
			switch (v) {
				case 'registered': return wlocI18n.t('Registered');
				case 'connecting': return wlocI18n.t('Connecting');
				case 'not_detected': return wlocI18n.t('Not detected');
				case 'likely_registered': return wlocI18n.t('Likely registered');
				case 'active_traffic': return wlocI18n.t('Active traffic');
				case 'nat_t_seen': return wlocI18n.t('NAT-T seen');
				case 'negotiating': return wlocI18n.t('Negotiating');
				case 'no_session': return wlocI18n.t('No session');
				default: return v || '-';
			}
		}
		function activityLabel(v) {
			switch (v) {
				case 'handshake_success': return wlocI18n.t('Handshake success');
				case 'handshake_failed': return wlocI18n.t('Handshake failed');
				case 'sustained_traffic': return wlocI18n.t('Sustained traffic');
				default: return v || '-';
			}
		}
		function meaningLabel(v) {
			switch (v) {
				case 'likely_call': return wlocI18n.t('Call in progress (inferred from sustained encrypted traffic)');
				default: return wlocI18n.t('Encrypted activity; call/SMS unknown');
			}
		}
		function lines(value) { return value.trim() ? value.trim().split('\n').reverse() : []; }

		/* ---------- 设备隧道状态 ---------- */
		var statusBody = E('tbody', {}, []);
		function statusRows(source) {
			return (source.devices || []).map(function(d) {
				var values = [d.label, d.ip, wfcLabel(d.wificalling || d.state), d.node || '-', d.epdg_ip || '-',
					(d.ike_seen ? '500' : '-') + ' / ' + (d.nat_t_seen ? '4500' : '-'),
					d.assured ? wlocI18n.t('Yes') : wlocI18n.t('No'), d.sent_packets + ' ↑ / ' + d.reply_packets + ' ↓', when(d.last_activity)];
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
			ui.showModal(wlocI18n.t('Clear activity log?'), [E('p', {}, wlocI18n.t('This permanently removes only the Wi-Fi Calling activity history. Settings and system logs are not affected.')),
				E('div', { class: 'right' }, [E('button', { class: 'btn', click: ui.hideModal }, wlocI18n.t('Cancel')),
				E('button', { class: 'btn cbi-button-negative', click: function() {
					clearLog('wfc').then(function() { renderLog(''); ui.hideModal(); }).catch(function(err) { ui.hideModal(); ui.addNotification(null, E('p', {}, wlocI18n.t('Unable to clear log: ') + ' ' + err.message), 'error'); });
				} }, wlocI18n.t('Clear log'))])]);
		} }, wlocI18n.t('Clear log'));

		/* ---------- 轮询刷新 ---------- */
		poll.add(function() {
			return L.resolveDefault(fs.read('/var/run/wificalling-gateway/status.json'), '{}').then(renderStatus);
		}, 5);
		poll.add(function() {
			return L.resolveDefault(fs.read('/var/run/wificalling-gateway/events.log'), '').then(renderLog);
		}, 5);

		var children = [
			E('h2', {}, wlocI18n.t('Wi-Fi Calling Monitor & Log')),
			E('div', { class: 'cbi-section' }, [
				E('h3', {}, wlocI18n.t('Device tunnel status')),
				E('p', {}, wlocI18n.t('Registered means an ASSURED bidirectional UDP 4500 tunnel was observed. This is network evidence, not carrier activation confirmation.')),
				E('table', { class: 'table' }, [
					E('tr', { class: 'tr table-titles' }, [wlocI18n.t('Device'), wlocI18n.t(wlocI18n.t('IP')), wlocI18n.t('Wi-Fi Calling status'), wlocI18n.t('Node'), wlocI18n.t(wlocI18n.t('ePDG IP')), wlocI18n.t(wlocI18n.t('UDP 500/4500')), wlocI18n.t(wlocI18n.t('ASSURED')), wlocI18n.t('Packets'), wlocI18n.t('Last activity')].map(function(x) { return E('th', { class: 'th' }, x); })),
					statusBody
				])
			]),
			E('div', { class: 'cbi-section' }, [
				E('h3', {}, wlocI18n.t('Encrypted IMS activity log')),
				E('p', {}, wlocI18n.t('Records handshake success or failure and sustained encrypted communication such as ringing or calls. Brief traffic bursts are not logged. The tunnel content is encrypted: a call is inferred from sustained bidirectional traffic, SMS cannot be distinguished, and phone numbers or message content are never visible.')),
				E('p', {}, [wlocI18n.t('Records: ') + ' ', logCount, ' ', clear]),
				E('table', { class: 'table' }, [
					E('tr', { class: 'tr table-titles' }, [wlocI18n.t('Time'), wlocI18n.t('Device'), wlocI18n.t(wlocI18n.t('IP')), wlocI18n.t('Wi-Fi Calling'), wlocI18n.t('Activity'), wlocI18n.t('Packet delta'), wlocI18n.t('Meaning')].map(function(x) { return E('th', { class: 'th' }, x); })),
					logBody
				])
			])
		];
		if (logEnabled === '0')
			children.splice(3, 0, E('div', { class: 'alert-message warning' }, wlocI18n.t('Activity log recording is disabled. Enable it in Settings.')));
		return E([], children);
	}
});
