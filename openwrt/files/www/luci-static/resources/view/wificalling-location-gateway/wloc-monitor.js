'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require fs';
'require poll';
'require dom';
'require ui';
'require uci';
'require rpc';

// WLOC 监控与日志：当前生效定位信息（含 GPS）+ 定位替换事件日志。
// 定位拦截设置见 "WLOC 设置" 页。

var STATUS_FILE = '/var/run/wloc-service/status.json';
var EVENTS_FILE = '/var/run/wloc-service/events.jsonl';
var PROFILE_STATUS_ROOT = '/var/run/wloc-service/profiles/';
var PROFILE_ID_RE = /^[a-z0-9_]{1,32}$/;

function profileFile(profileId, filename) {
	if (profileId === 'default') return filename === 'status.json' ? STATUS_FILE : EVENTS_FILE;
	if (!PROFILE_ID_RE.test(profileId)) return null;
	return PROFILE_STATUS_ROOT + profileId + '/' + filename;
}

function readProfileState(profileId) {
	var statusPath = profileFile(profileId, 'status.json');
	var eventsPath = profileFile(profileId, 'events.jsonl');
	if (!statusPath || !eventsPath) return Promise.reject(new Error('invalid profile id'));
	return Promise.all([
		L.resolveDefault(fs.read(statusPath), '{}'),
		L.resolveDefault(fs.read(eventsPath), '')
	]).then(function(values) {
		var parsed;
		try { parsed = JSON.parse(values[0]); } catch (e) { parsed = {}; }
		return { status: parsed, events: values[1] || '' };
	});
}

// 手动刷新按钮的忙碌状态；轮询重渲染表格时据此保持按钮为“刷新中”。
var refreshingIp = false;

var callCtl = rpc.declare({
	object: 'luci.wloc',
	method: 'ctl',
	params: [ 'method', 'query', 'lat', 'lon' ]
});

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
			uci.load('wloc-service')
		]);
	},

	render: function(data) {
		wlocI18n.localizeTabs();
		var status;
		try { status = JSON.parse(data[0]); } catch (e) { status = {}; }
		var eventsText = data[1] || '';
		var geo = status.geo || {};
		var selectedProfile = 'default';
		var profiles = uci.sections('wloc-service', 'device');

		/* ---------- 当前定位 ---------- */
		var geoBody = E('tbody', {}, []);
		var profileSelector = E('select', { 'class': 'cbi-input-select' });
		profileSelector.appendChild(E('option', { value: 'default' }, wlocI18n.t('Default / legacy profile')));
		profiles.forEach(function(profile) {
			if (!PROFILE_ID_RE.test(profile['.name'])) return;
			profileSelector.appendChild(E('option', { value: profile['.name'] },
				(profile.label || profile['.name']) + ' (' + profile['.name'] + ')'));
		});
		// 手动刷新 IP：通知守护进程丢弃缓存的出口探测结果并立即重新
		// 探测（切换设备节点后无需等待周期巡检），完成后重读状态文件。
		function refreshIpBtn() {
			var label = refreshingIp ? wlocI18n.t('Refreshing…') : wlocI18n.t('Refresh IP');
			return E('button', {
				class: 'cbi-button cbi-button-apply',
				disabled: refreshingIp ? true : undefined,
				title: wlocI18n.t('Re-probe the followed node exit IP now'),
				click: function() {
					if (refreshingIp) return;
					refreshingIp = true;
					renderGeo(status);
					callCtl('refresh', null, null, null).then(function() {
						// The daemon re-probes and rewrites status.json
						// before replying; read it once so the rows update
						// immediately instead of on the next poll tick.
						return L.resolveDefault(fs.read(profileFile(selectedProfile, 'status.json')), '{}');
					}).then(function(text) {
						var fresh;
						try { fresh = JSON.parse(text); } catch (e) { fresh = status; }
						refreshingIp = false;
						renderGeo(fresh);
					}).catch(function(err) {
						refreshingIp = false;
						renderGeo(status);
						ui.addNotification(null, E('p', {}, wlocI18n.t('IP refresh failed: ') + ' ' + (err.message || err)), 'error');
					});
				}
			}, label);
		}
		function geoRows(s) {
			var g = s.geo || {};
			var deviceLabel = '-';
			if (s.assigned_device) {
				// assigned_device is the profile's canonical device address.
				var dev = uci.sections('wloc-service', 'device').find(function(d) {
					return d.assigned_device === s.assigned_device;
				});
				deviceLabel = (dev && dev.label ? dev.label : s.assigned_device) + ' (' + s.assigned_device + ')';
			}
			return [
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Service phase')), E('td', { class: 'td' }, phaseLabel(s.service_phase))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Follow device')), E('td', { class: 'td' }, deviceLabel), E('td', { class: 'td right' }, refreshIpBtn())]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Location mode')), E('td', { class: 'td' }, sourceLabel(s.geo_source))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Country')), E('td', { class: 'td' }, g.country_code || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('City')), E('td', { class: 'td' }, g.city || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Timezone')), E('td', { class: 'td' }, g.timezone || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('GPS (lat / lon)')), E('td', { class: 'td' }, gpsOf(g))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Geo state')), E('td', { class: 'td' }, g.state || '-')]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Observed at')), E('td', { class: 'td' }, fmtTime(s.observed_at))]),
				E('tr', { class: 'tr' }, [E('td', { class: 'td' }, wlocI18n.t('Exit IP')), E('td', { class: 'td' }, (s.exit && s.exit.ip) ? s.exit.ip : ((s.exit && s.exit.last_error) ? wlocI18n.t(s.exit.last_error) : '-'))])
			];
		}
		function renderGeo(s) { dom.content(geoBody, geoRows(s)); }
		renderGeo(status);
		profileSelector.onchange = function() {
			var next = this.value;
			if (next !== 'default' && !PROFILE_ID_RE.test(next)) return;
			selectedProfile = next;
			readProfileState(selectedProfile).then(function(state) {
				status = state.status;
				eventsText = state.events;
				renderGeo(status);
				renderLog(eventsText);
			}).catch(function(error) {
				ui.addNotification(null, E('p', {}, wlocI18n.t('Profile state unavailable: ') + error.message), 'error');
			});
		};

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
		function eventTime(event) { return event.timestamp || event.time; }
		function logRows(events) {
			return events.map(function(ev) {
				var fields = ev.fields || {};
				var where = fields.city || fields.country_code
					? (fields.city || '') + (fields.country_code ? ' (' + fields.country_code + ')' : '') : '-';
				return E('tr', { class: 'tr' }, [
					E('td', { class: 'td' }, fmtTime(eventTime(ev))),
					E('td', { class: 'td' }, eventLabel(ev.event_code || ev.type)),
					E('td', { class: 'td' }, where),
					E('td', { class: 'td' }, sourceLabel(fields.source || ev.source))
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
					fs.write(profileFile(selectedProfile, 'events.jsonl'), '').then(function() {
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
			return readProfileState(selectedProfile).then(function(state) {
				status = state.status;
				eventsText = state.events;
				renderGeo(status);
				renderLog(eventsText);
			});
		}, 5);

		return E([], [
			E('h2', {}, wlocI18n.t('WLOC Monitor & Log')),
			E('div', { 'class': 'cbi-section' }, [
				E('label', { 'class': 'cbi-value-title' }, wlocI18n.t('Device profile')), ' ', profileSelector,
				E('p', { style: 'color:#666;font-size:12px' }, wlocI18n.t('Each profile has independent location state and bounded events.'))
			]),
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
