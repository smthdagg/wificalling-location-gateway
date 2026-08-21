'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require uci';
'require rpc';
'require poll';
'require ui';

// v2 device profiles: one stable local id binds one LAN device to one node
// policy and one WLOC mode. Configuration is UCI-backed; runtime state is
// read from the bounded redacted health projection.

var getHealth = rpc.declare({ object: 'luci.wloc', method: 'health' });
var restartUnified = rpc.declare({ object: 'luci.wloc', method: 'restart_unified' });

function notify(title, message, kind) {
	ui.addNotification(null, E('p', [ E('strong', title + ': '), message ]), kind || 'info');
}

function statusText(status) {
	if (!status) return wlocI18n.t('Not observed');
	return status.phase + ' (' + status.reason_code + ')';
}

return view.extend({
	load: function() {
		return Promise.all([
			uci.load('wloc-service'),
			L.resolveDefault(getHealth(), {})
		]);
	},

	render: function(data) {
		wlocI18n.localizeTabs();
		var body = E('tbody', {});
		var health = data[1] || {};
		var profileStatus = {};

		function indexHealth(h) {
			profileStatus = {};
			(h.profiles || []).forEach(function(p) { profileStatus[p.id] = p; });
		}
		indexHealth(health);

		function input(value, placeholder) {
			return E('input', {
				'class': 'cbi-input-text', 'value': value || '',
				'placeholder': placeholder || ''
			});
		}

		function saveProfile(section, fields) {
			var id = section['.name'];
			if (!/^[a-z0-9_-]{1,32}$/.test(id)) {
				notify(wlocI18n.t('Save failed'), wlocI18n.t('Profile id must use lowercase letters, numbers, - or _.') ,'error');
				return;
			}
			var label = fields.label.value.trim();
			var address = fields.address.value.trim();
			var node = fields.node.value.trim();
			if (!label || !address || !node) {
				notify(wlocI18n.t('Save failed'), wlocI18n.t('Label, device address, and node are required.'), 'error');
				return;
			}
			uci.set('wloc-service', id, 'label', label);
			uci.set('wloc-service', id, 'assigned_device', address);
			uci.set('wloc-service', id, 'node_ref', node);
			uci.set('wloc-service', id, 'node_mode', fields.nodeMode.value);
			uci.set('wloc-service', id, 'geo_source', fields.geoMode.value);
			uci.set('wloc-service', id, 'enabled', fields.enabled.checked ? '1' : '0');
			uci.set('wloc-service', id, 'manual_lat', fields.latitude.value.trim());
			uci.set('wloc-service', id, 'manual_lon', fields.longitude.value.trim());
			uci.save('wloc-service').then(function() {
				return ui.changes.apply(true);
			}).then(function() {
				return restartUnified();
			}).then(function(result) {
				if (result && result.error) throw new Error(result.error);
				notify(wlocI18n.t('Saved'), wlocI18n.t('Unified Gateway/WLOC supervisor restarted.'));
				return getHealth();
			}).then(function(result) {
				indexHealth(result || {});
				renderRows();
			}).catch(function(error) {
				notify(wlocI18n.t('Save failed'), String(error), 'error');
			});
		}

		function removeProfile(section) {
			var id = section['.name'];
			if (!window.confirm(wlocI18n.t('Delete profile %s?').format(id))) return;
			uci.delete('wloc-service', id);
			uci.save('wloc-service').then(function() {
				return ui.changes.apply(true);
			}).then(function() {
				return restartUnified();
			}).then(function() {
				notify(wlocI18n.t('Deleted'), wlocI18n.t('Profile removed and its redirect was withdrawn.'));
				return uci.load('wloc-service');
			}).then(renderRows).catch(function(error) {
				notify(wlocI18n.t('Delete failed'), String(error), 'error');
			});
		}

		function renderRows() {
			body.innerHTML = '';
			var profiles = uci.sections('wloc-service', 'device');
			if (!profiles.length) {
				body.appendChild(E('tr', {}, [E('td', { 'colspan': 8 }, wlocI18n.t('No v2 device profiles yet.'))]));
				return;
			}
			profiles.forEach(function(section) {
				var fields = {
					label: input(section.label, wlocI18n.t('Label')),
					address: input(section.assigned_device, '192.168.1.100 or MAC'),
					node: input(section.node_ref || 'default', 'node tag'),
					latitude: input(section.manual_lat, 'lat'),
					longitude: input(section.manual_lon, 'lon'),
					enabled: E('input', { 'type': 'checkbox', 'checked': section.enabled === '1' })
				};
				fields.nodeMode = E('select', {});
				[['fixed', 'Fixed'], ['gateway_default', 'Gateway default']].forEach(function(option) {
					fields.nodeMode.appendChild(E('option', { value: option[0], selected: (section.node_mode || 'fixed') === option[0] }, option[1]));
				});
				fields.geoMode = E('select', {});
				[['auto', 'Auto follow'], ['manual', 'Manual']].forEach(function(option) {
					fields.geoMode.appendChild(E('option', { value: option[0], selected: (section.geo_source || 'auto') === option[0] }, option[1]));
				});
				var state = profileStatus[section['.name']];
				var actions = E('span', {}, [
					E('button', { 'class': 'cbi-button cbi-button-apply', click: function() { saveProfile(section, fields); } }, wlocI18n.t('Save')),
					' ',
					E('button', { 'class': 'cbi-button cbi-button-remove', click: function() { removeProfile(section); } }, wlocI18n.t('Delete'))
				]);
				body.appendChild(E('tr', { 'class': 'tr' }, [
					E('td', { 'class': 'td' }, section['.name']),
					E('td', { 'class': 'td' }, fields.label),
					E('td', { 'class': 'td' }, fields.address),
					E('td', { 'class': 'td' }, fields.node),
					E('td', { 'class': 'td' }, [fields.nodeMode, ' / ', fields.geoMode]),
					E('td', { 'class': 'td' }, [fields.latitude, ' ', fields.longitude]),
					E('td', { 'class': 'td' }, [fields.enabled, ' ', statusText(state)]),
					E('td', { 'class': 'td' }, actions)
				]));
			});
		}

		function addProfile() {
			var id = window.prompt(wlocI18n.t('New profile id'), 'phone');
			if (!id || !/^[a-z0-9_-]{1,32}$/.test(id) || uci.get('wloc-service', id)) {
				notify(wlocI18n.t('Add failed'), wlocI18n.t('Use a unique lowercase profile id.'), 'error');
				return;
			}
			uci.add('wloc-service', 'device', id);
			uci.set('wloc-service', id, 'label', id);
			uci.set('wloc-service', id, 'node_ref', 'default');
			uci.set('wloc-service', id, 'node_mode', 'fixed');
			uci.set('wloc-service', id, 'geo_source', 'auto');
			uci.set('wloc-service', id, 'enabled', '0');
			uci.save('wloc-service').then(function() { return ui.changes.apply(true); }).then(function() {
				return uci.load('wloc-service');
			}).then(renderRows).catch(function(error) {
				notify(wlocI18n.t('Add failed'), String(error), 'error');
			});
		}

		renderRows();
		poll.add(function() {
			return L.resolveDefault(getHealth(), {}).then(function(result) {
				indexHealth(result || {});
				renderRows();
			});
		}, 5);

		return E([], [
			E('h2', {}, wlocI18n.t('Device profiles')),
			E('p', {}, wlocI18n.t('Each profile owns one device binding, node selection, WLOC auto/manual location mode, enable state, and isolated redirect. Status is redacted in health output; addresses remain local to this settings page.')),
			E('p', {}, E('button', { 'class': 'cbi-button cbi-button-add', click: addProfile }, wlocI18n.t('Add profile'))),
			E('div', { 'class': 'cbi-section', style: 'overflow:auto' }, E('table', { 'class': 'table' }, [
				E('tr', { 'class': 'tr table-titles' }, [
					'ID', wlocI18n.t('Label'), wlocI18n.t('Device'), wlocI18n.t('Node'), wlocI18n.t('Mode'), wlocI18n.t('Manual location'), wlocI18n.t('Enabled / state'), ''
				].map(function(title) { return E('th', { 'class': 'th' }, title); })),
				body
			]))
		]);
	}
});
