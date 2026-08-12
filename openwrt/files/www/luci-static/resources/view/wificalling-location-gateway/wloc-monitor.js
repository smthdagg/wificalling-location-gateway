'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require wificalling-location-gateway.tabs as wlocTabs';
'require fs';
'require poll';
'require dom';
'require ui';
'require uci';

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

// Service phase is a technical status: keep the raw English values
// (intercepting / disabled / ...) as-is.
function phaseLabel(phase) {
	return phase || '-';
}

function sourceLabel(v) {
	return v === 'manual' ? wlocI18n.t('Manual') : (v === 'auto' ? wlocI18n.t('Auto') : (v || '-'));
}

function eventLabel(v) {
	switch (v) {
		case 'target_updated': return wlocI18n.t('Target updated');
		case 'rewritten': return wlocI18n.t('WLOC response rewritten');
		default: return v || '-';
	}
}

return view.extend({
	load: function() {
		return Promise.all([
			L.resolveDefault(fs.read(STATUS_FILE), '{}'),
			L.resolveDefault(fs.read(EVENTS_FILE), ''),
			uci.load('wificalling-gateway')
		]);
	},

	render: function(data) {
		wlocI18n.localizeTabs();
		wlocTabs.localize();
		var status;
		try { status = JSON.parse(data[0]); } catch (e) { status = {}; }
		var eventsText = data[1] || '';
		var geo = status.geo || {};

		/* ---------- 当前定位 ---------- */
		var geoBody = E('tbody', {}, []);
		function geoRows(s) {
			var g = s.geo || {};
			var deviceLabel = '-';
			if (s.assigned_device) {
				// source_ip is a DynamicList value (array) on the device policy.
				var dev = uci.sections('wificalling-gateway', 'device').find(function(d) {
					return (d.source_ip || []).indexOf(s.assigned_device) >= 0;
				});
				deviceLabel = (dev && dev.label ? dev.label : s.assigned_device) + ' (' + s.assigned_device + ')';
			}
			return [
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Service phase')), E('td', { class: 'td' }, phaseLabel(s.service_phase))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Follow device')), E('td', { class: 'td' }, deviceLabel)]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Location mode')), E('td', { class: 'td' }, sourceLabel(s.geo_source))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Country')), E('td', { class: 'td' }, g.country_code || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('City')), E('td', { class: 'td' }, g.city || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Timezone')), E('td', { class: 'td' }, g.timezone || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('GPS (lat / lon)')), E('td', { class: 'td' }, gpsOf(g))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Geo state')), E('td', { class: 'td' }, g.state || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Observed at')), E('td', { class: 'td' }, fmtTime(s.observed_at))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Exit IP')), E('td', { class: 'td' }, (s.exit && s.exit.ip) ? s.exit.ip : '-')])
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
			ui.showModal(wlocI18n.t('Clear WLOC usage log?'), [E('p', {}, wlocI18n.t('This clears the local history of WLOC location events. Location interception settings are not affected.')),
				E('div', { class: 'right' }, [E('button', { class: 'btn', click: ui.hideModal }, wlocI18n.t('Cancel')),
				E('button', { class: 'btn cbi-button-negative', click: function() {
					fs.write(EVENTS_FILE, '').then(function() {
						renderLog('');
						ui.hideModal();
						ui.addNotification(null, E('p', {}, wlocI18n.t('WLOC usage log cleared.')), 'info');
					}).catch(function(err) {
						ui.addNotification(null, E('p', {}, wlocI18n.t('Unable to clear log: ') + ' ' + err.message), 'error');
					});
				} }, wlocI18n.t('Clear log'))])]);
		} }, wlocI18n.t('Clear log'));

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
			E('h2', {}, wlocI18n.t('WLOC Monitor & Log')),
			E('div', { 'class': 'cbi-section' }, [
				E('h3', {}, wlocI18n.t('Current location')),
				E('p', {}, wlocI18n.t('Shows the effective location target: auto follows the node exit, manual uses the coordinates from the settings page. GPS values stay on this router.')),
				E('table', { class: 'table' }, geoBody)
			]),
			E('div', { 'class': 'cbi-section' }, [
				E('h3', {}, wlocI18n.t('WLOC usage log')),
				E('p', {}, wlocI18n.t('Records each location target update (time, place, source auto/manual). Raw WLOC responses are never recorded.')),
				E('p', {}, [wlocI18n.t('Records: ') + ' ', logCount, ' ', clearLog]),
				E('table', { class: 'table' }, [
					E('tr', { class: 'tr table-titles' }, [wlocI18n.t('Time'), wlocI18n.t('Event'), wlocI18n.t('Location'), wlocI18n.t('Source')].map(function(x) { return E('th', { class: 'th' }, x); })),
					logBody
				])
			])
		]);
	}
});
