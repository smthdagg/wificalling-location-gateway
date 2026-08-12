'use strict';
'require view';
'require fs';
'require poll';
'require dom';
'require ui';

// WLOC 监控与日志：当前生效定位信息（含 GPS）+ 定位替换事件日志。
// 定位拦截设置见 "WLOC 设置" 页。

var STATUS_FILE = '/var/run/wloc-service/status.json';
var EVENTS_FILE = '/var/run/wloc-service/events.jsonl';

function fmtTime(unix) {
	if (!unix) return '-';
	return new Date(unix * 1000).toLocaleString();
}

function gpsOf(geo) {
	return (geo && geo.latitude != null && geo.longitude != null)
		? geo.latitude.toFixed(6) + ' / ' + geo.longitude.toFixed(6) : '-';
}

function phaseLabel(phase) {
	switch (phase) {
		case 'disabled': return _('已禁用');
		case 'starting': return _('启动中');
		case 'ready_passthrough': return _('就绪（透传）');
		case 'intercepting': return _('拦截中');
		case 'degraded_passthrough': return _('降级（透传）');
		case 'draining': return _('排空中');
		default: return phase || '-';
	}
}

function sourceLabel(v) {
	return v === 'manual' ? _('手动') : (v === 'auto' ? _('自动') : (v || '-'));
}

function eventLabel(v) {
	switch (v) {
		case 'target_updated': return _('定位目标更新');
		case 'rewritten': return _('WLOC 响应已重写');
		default: return v || '-';
	}
}

return view.extend({
	load: function() {
		return Promise.all([
			L.resolveDefault(fs.read(STATUS_FILE), '{}'),
			L.resolveDefault(fs.read(EVENTS_FILE), '')
		]);
	},

	render: function(data) {
		var status;
		try { status = JSON.parse(data[0]); } catch (e) { status = {}; }
		var eventsText = data[1] || '';
		var geo = status.geo || {};

		/* ---------- 当前定位 ---------- */
		var geoBody = E('tbody', {}, []);
		function geoRows(s) {
			var g = s.geo || {};
			return [
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, _('服务阶段')), E('td', { class: 'td' }, phaseLabel(s.service_phase))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, _('定位模式')), E('td', { class: 'td' }, sourceLabel(s.geo_source))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, _('国家/地区')), E('td', { class: 'td' }, g.country_code || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, _('城市')), E('td', { class: 'td' }, g.city || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, _('时区')), E('td', { class: 'td' }, g.timezone || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, _('GPS 坐标（纬度 / 经度）')), E('td', { class: 'td' }, gpsOf(g))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, _('定位状态')), E('td', { class: 'td' }, g.state || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, _('观测时间')), E('td', { class: 'td' }, fmtTime(s.observed_at))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, _('出口 IP')), E('td', { class: 'td' }, (s.exit && s.exit.ip) ? s.exit.ip : '-')])
			];
		}
		function renderGeo(s) { dom.content(geoBody, geoRows(s)); }
		renderGeo(status);

		/* ---------- 使用日志 ---------- */
		var logBody = E('tbody', {}, []);
		function parseEvents(text) {
			var rows = [];
			text.split('\n').forEach(function(line) {
				line = line.trim();
				if (!line) return;
				try { rows.push(JSON.parse(line)); } catch (e) {}
			});
			return rows.slice(-20).reverse();
		}
		function logRows(events) {
			return events.map(function(ev) {
				var where = '-';
				if (ev.city || ev.country_code)
					where = (ev.city || '') + (ev.country_code ? ' (' + ev.country_code + ')' : '');
				else if (ev.latitude != null && ev.longitude != null)
					where = ev.latitude.toFixed(4) + ', ' + ev.longitude.toFixed(4);
				return E('tr', { class: 'tr' }, [
					E('td', { class: 'td' }, fmtTime(ev.time)),
					E('td', { class: 'td' }, eventLabel(ev.type)),
					E('td', { class: 'td' }, where),
					E('td', { class: 'td' }, sourceLabel(ev.source))
				]);
			});
		}
		var logCount = E('span', {}, String(parseEvents(eventsText).length));
		function renderLog(text) { dom.content(logBody, logRows(parseEvents(text))); dom.content(logCount, String(parseEvents(text).length)); }
		renderLog(eventsText);

		var clearLog = E('button', { class: 'btn cbi-button-negative', click: function() {
			ui.showModal(_('清空 WLOC 使用日志？'), [E('p', {}, _('此操作将清空 WLOC 定位事件的本地历史记录。定位拦截设置不受影响。')),
				E('div', { class: 'right' }, [E('button', { class: 'btn', click: ui.hideModal }, _('取消')),
				E('button', { class: 'btn cbi-button-negative', click: function() {
					fs.write(EVENTS_FILE, '').then(function() {
						renderLog('');
						ui.hideModal();
						ui.addNotification(null, E('p', {}, _('WLOC 使用日志已清空。')), 'info');
					}).catch(function(err) {
						ui.addNotification(null, E('p', {}, _('无法清空日志：') + ' ' + err.message), 'error');
					});
				} }, _('清空日志'))])]);
		} }, _('清空日志'));

		/* ---------- 轮询刷新 ---------- */
		poll.add(function() {
			return L.resolveDefault(fs.read(STATUS_FILE), '{}').then(function(v) {
				var s;
				try { s = JSON.parse(v); } catch (e) { return; }
				renderGeo(s);
			});
		}, 5);
		poll.add(function() {
			return L.resolveDefault(fs.read(EVENTS_FILE), '').then(renderLog);
		}, 5);

		return E([], [
			E('h2', {}, _('WLOC 监控与日志')),
			E('div', { 'class': 'cbi-section' }, [
				E('h3', {}, _('当前定位')),
				E('p', {}, _('显示当前生效的定位目标：自动模式跟随节点出口，手动模式使用设置页的坐标。GPS 数值只保留在本路由器上。')),
				E('table', { class: 'table' }, geoBody)
			]),
			E('div', { 'class': 'cbi-section' }, [
				E('h3', {}, _('WLOC 使用日志')),
				E('p', {}, _('记录每次定位目标更新（时间、目标位置、来源 自动/手动）。不记录原始 WLOC 响应内容。')),
				E('p', {}, [_('记录数：') + ' ', logCount, ' ', clearLog]),
				E('table', { class: 'table' }, [
					E('tr', { class: 'tr table-titles' }, [_('时间'), _('事件'), _('位置'), _('来源')].map(function(x) { return E('th', { class: 'th' }, x); })),
					logBody
				])
			])
		]);
	}
});
