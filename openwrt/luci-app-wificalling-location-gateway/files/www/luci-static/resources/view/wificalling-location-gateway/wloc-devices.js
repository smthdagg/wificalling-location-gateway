'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require uci';
'require rpc';
'require poll';
'require ui';

// Unified V2 management page. Changes are staged in LuCI's UCI buffer and
// cross the single save/apply/restart boundary below. Health is a bounded
// projection; this page never displays node credentials or raw traffic.
var getHealth = rpc.declare({ object: 'luci.wloc', method: 'health' });
var restartUnified = rpc.declare({ object: 'luci.wloc', method: 'restart_unified' });
var MAX_PROFILES = 8;

function notify(title, message, kind) {
	ui.addNotification(null, E('p', [ E('strong', title + ': '), message ]), kind || 'info');
}

function profileStatus(status) {
	if (!status) return wlocI18n.t('Not observed');
	return (status.phase || 'unknown') + ' (' + (status.reason_code || 'unknown') + ')';
}

function normalizeDeviceAddress(value) { var address = String(value || '').trim().toLowerCase(), octets = address.split('.'); if (octets.length === 4 && octets.every(function(octet) { return /^\d{1,3}$/.test(octet) && Number(octet) <= 255; })) { var numbers = octets.map(Number), first = numbers[0]; if (first === 10 || (first === 172 && numbers[1] >= 16 && numbers[1] <= 31) || (first === 192 && numbers[1] === 168)) return numbers.join('.'); } if (/^([0-9a-f]{2}:){5}[0-9a-f]{2}$/.test(address) || /^([0-9a-f]{2}-){5}[0-9a-f]{2}$/.test(address)) { var hex = address.replace(/[:-]/g, ''); if (!/^0+$/.test(hex) && !(parseInt(hex.slice(0, 2), 16) & 1)) return hex; } return null; }

function profileFromFields(section, fields) {
	return { '.name': section['.name'], label: fields.label.value.trim(), assigned_device: fields.address.value.trim(), node_ref: fields.node.value.trim(), node_mode: fields.nodeMode.value, geo_source: fields.geoMode.value, manual_lat: fields.latitude.value.trim(), manual_lon: fields.longitude.value.trim(), enabled: fields.enabled.checked ? '1' : '0' };
}

function validateProfile(profile) {
	if (!/^[a-z0-9_-]{1,32}$/.test(profile['.name'])) return 'invalid profile id';
	if (!profile.label || profile.label.length > 48) return 'label must be 1-48 characters';
	if (!profile.assigned_device || profile.assigned_device.length > 64 || profile.assigned_device !== profile.assigned_device.trim() || !normalizeDeviceAddress(profile.assigned_device)) return 'device address must be a private IPv4 address or unicast MAC';
	if (!profile.node_ref || profile.node_ref.length > 96) return 'node reference is required and bounded';
	if (['fixed', 'gateway_default'].indexOf(profile.node_mode || 'fixed') < 0) return 'invalid node mode';
	if (['auto', 'manual'].indexOf(profile.geo_source || 'auto') < 0) return 'invalid location mode';
	if (profile.geo_source === 'manual') { var lat = Number(profile.manual_lat), lon = Number(profile.manual_lon); if (!profile.manual_lat || !profile.manual_lon || !isFinite(lat) || !isFinite(lon) || lat < -90 || lat > 90 || lon < -180 || lon > 180) return 'manual coordinates are invalid'; }
	return null;
}

		function validateProfiles(profiles) {
	if (profiles.length > MAX_PROFILES) return ['at most ' + MAX_PROFILES + ' profiles are supported'];
	var ids = {}, devices = {}, errors = [];
	profiles.forEach(function(profile) { var error = validateProfile(profile), device = normalizeDeviceAddress(profile.assigned_device) || String(profile.assigned_device || '').toLowerCase(); if (ids[profile['.name']]) error = 'duplicate profile id'; if (devices[device]) error = 'each device may have only one profile'; ids[profile['.name']] = true; devices[device] = true; if (error) errors.push(profile['.name'] + ': ' + error); });
	return errors;
}

return view.extend({
	load: function() { return Promise.all([uci.load('wloc-service'), L.resolveDefault(getHealth(), {})]); },

	render: function(data) {
		wlocI18n.localizeTabs();
		var health = data[1] || {}, body = E('tbody', {}), stateCells = {}, healthSummary = E('span', {}), basic = {};
		function textInput(value, placeholder, type) { return E('input', { class: 'cbi-input-text', type: type || 'text', value: value || '', placeholder: placeholder || '' }); }
		function refreshHealth(current) { current = current || {}; var services = (current || {}).services || {}, wloc = services.wloc || {}, gateway = services.gateway || {}; healthSummary.textContent = 'Gateway: ' + (gateway.running ? 'running' : 'stopped') + ' | WLOC: ' + (wloc.running ? 'running' : 'stopped') + ' | ' + (wloc.phase || 'unknown'); (current.profiles || []).forEach(function(profile) { if (stateCells[profile.id]) stateCells[profile.id].textContent = profileStatus(profile); }); }
		function setOption(select, values, selected) { values.forEach(function(option) { select.appendChild(E('option', { value: option[0], selected: option[0] === selected }, option[1])); }); }
		function stageBasic() { var interval = Number(basic.interval.value); if (!isFinite(interval) || interval < 30 || interval > 86400 || Math.floor(interval) !== interval) { notify(wlocI18n.t('Apply failed'), 'probe interval must be between 30 and 86400 seconds', 'error'); return false; } if (['http', 'stub'].indexOf(basic.provider.value) < 0) { notify(wlocI18n.t('Apply failed'), 'geo provider is invalid', 'error'); return false; } uci.set('wloc-service', 'main', 'enabled', basic.enabled.checked ? '1' : '0'); uci.set('wloc-service', 'main', 'probe_interval', String(interval)); uci.set('wloc-service', 'main', 'geo_provider', basic.provider.value); return true; }
		function stageProfile(section, fields) { var profile = profileFromFields(section, fields), error = validateProfile(profile); if (error) { notify(wlocI18n.t('Stage failed'), profile['.name'] + ': ' + error, 'error'); return; } Object.keys(profile).forEach(function(key) { if (key !== '.name') uci.set('wloc-service', profile['.name'], key, profile[key]); }); notify(wlocI18n.t('Staged'), wlocI18n.t('Changes will take effect after Apply & restart.')); }
		function applyAll() { if (!stageBasic()) return Promise.resolve(false); var errors = validateProfiles(uci.sections('wloc-service', 'device')); if (errors.length) { notify(wlocI18n.t('Apply failed'), errors.join('; '), 'error'); return Promise.resolve(false); } return uci.save('wloc-service').then(function() { return ui.changes.apply(true); }).then(function() { return restartUnified(); }).then(function(result) { if (result && result.error) throw new Error(result.error); notify(wlocI18n.t('Applied'), wlocI18n.t('Unified Gateway/WLOC supervisor restarted.')); return L.resolveDefault(getHealth(), {}).then(refreshHealth); }).catch(function(error) { notify(wlocI18n.t('Apply failed'), String(error), 'error'); return false; }); }
		function renderRows() { body.innerHTML = ''; stateCells = {}; var profiles = uci.sections('wloc-service', 'device'); if (!profiles.length) { body.appendChild(E('tr', {}, E('td', { colspan: 8 }, wlocI18n.t('No device profiles yet.')))); return; } profiles.forEach(function(section) { var fields = { label: textInput(section.label, wlocI18n.t('Label')), address: textInput(section.assigned_device, '192.168.1.100 or MAC'), node: textInput(section.node_ref || 'default', 'node reference'), latitude: textInput(section.manual_lat, 'lat'), longitude: textInput(section.manual_lon, 'lon'), enabled: E('input', { type: 'checkbox', checked: section.enabled === '1' }), nodeMode: E('select', {}), geoMode: E('select', {}) }; setOption(fields.nodeMode, [['fixed', 'Fixed'], ['gateway_default', 'Gateway default']], section.node_mode || 'fixed'); setOption(fields.geoMode, [['auto', 'Auto follow'], ['manual', 'Manual']], section.geo_source || 'auto'); var state = E('span', {}); stateCells[section['.name']] = state; state.textContent = profileStatus((health.profiles || []).filter(function(p) { return p.id === section['.name']; })[0]); body.appendChild(E('tr', { class: 'tr' }, [E('td', { class: 'td' }, section['.name']), E('td', { class: 'td' }, fields.label), E('td', { class: 'td' }, fields.address), E('td', { class: 'td' }, fields.node), E('td', { class: 'td' }, [fields.nodeMode, ' / ', fields.geoMode]), E('td', { class: 'td' }, [fields.latitude, ' ', fields.longitude]), E('td', { class: 'td' }, [fields.enabled, ' ', state]), E('td', { class: 'td' }, [E('button', { class: 'cbi-button cbi-button-apply', click: function() { stageProfile(section, fields); } }, wlocI18n.t('Stage')), ' ', E('button', { class: 'cbi-button cbi-button-remove', click: function() { removeProfile(section); } }, wlocI18n.t('Delete'))])])); }); }
		function addProfile() { var profiles = uci.sections('wloc-service', 'device'); if (profiles.length >= MAX_PROFILES) return notify(wlocI18n.t('Add failed'), 'at most ' + MAX_PROFILES + ' profiles are supported', 'error'); var id = window.prompt(wlocI18n.t('New profile id'), 'device' + (profiles.length + 1)); if (!id || !/^[a-z0-9_-]{1,32}$/.test(id) || uci.get('wloc-service', id)) return notify(wlocI18n.t('Add failed'), wlocI18n.t('Use a unique lowercase profile id.'), 'error'); uci.add('wloc-service', 'device', id); [['label', id], ['node_ref', 'default'], ['node_mode', 'fixed'], ['geo_source', 'auto'], ['enabled', '0']].forEach(function(pair) { uci.set('wloc-service', id, pair[0], pair[1]); }); renderRows(); }
		function removeProfile(section) { if (!window.confirm(wlocI18n.t('Delete profile %s?').format(section['.name']))) return; uci.delete('wloc-service', section['.name']); renderRows(); }
		var main = uci.get('wloc-service', 'main') || {}; basic.enabled = E('input', { type: 'checkbox', checked: main.enabled !== '0' }); basic.interval = textInput(main.probe_interval || '300', '30-86400', 'number'); basic.provider = E('select', {}); setOption(basic.provider, [['http', 'HTTP provider'], ['stub', 'Stub provider']], main.geo_provider || 'http'); var applyButton = E('button', { class: 'cbi-button cbi-button-apply', click: applyAll }, wlocI18n.t('Apply & restart'));
		renderRows(); refreshHealth(health); poll.add(function() { return L.resolveDefault(getHealth(), {}).then(refreshHealth); }, 15);
		return E([], [E('h2', {}, wlocI18n.t('Unified Gateway / WLOC')), E('p', {}, wlocI18n.t('Basic settings, device profiles, node selection, WLOC location mode, and service state share one apply boundary.')), E('div', { class: 'cbi-section' }, [E('h3', {}, wlocI18n.t('Basic settings')), E('p', {}, healthSummary), E('div', { class: 'cbi-row' }, [E('label', {}, wlocI18n.t('Enable unified service')), ' ', basic.enabled]), E('div', { class: 'cbi-row' }, [E('label', {}, wlocI18n.t('Probe interval (seconds)')), ' ', basic.interval]), E('div', { class: 'cbi-row' }, [E('label', {}, wlocI18n.t('Geo provider')), ' ', basic.provider])]), E('div', { class: 'cbi-section', style: 'overflow:auto' }, [E('h3', {}, wlocI18n.t('Device profiles')), E('p', {}, [E('button', { class: 'cbi-button cbi-button-add', click: addProfile }, wlocI18n.t('Add profile')), ' ', applyButton]), E('table', { class: 'table' }, [E('tr', { class: 'tr table-titles' }, ['ID', wlocI18n.t('Label'), wlocI18n.t('Device'), wlocI18n.t('Node'), wlocI18n.t('Mode'), wlocI18n.t('Manual location'), wlocI18n.t('Enabled / state'), wlocI18n.t('Action')].map(function(title) { return E('th', { class: 'th' }, title); })), body])])]);
	}
});
