'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require uci';
'require rpc';
'require poll';
'require ui';

var health = rpc.declare({ object: 'luci.wloc', method: 'health' });
var restart = rpc.declare({ object: 'luci.wloc', method: 'restart_unified' });

function note(title, message, kind) {
	ui.addNotification(null, E('p', [E('strong', title + ': '), message]), kind || 'info');
}

return view.extend({
	load: function() {
		return Promise.all([uci.load('wloc-service'), L.resolveDefault(health(), {})]);
	},
	render: function(data) {
		var body = E('tbody', {}), states = {};
		function index(value) {
			states = {};
			(value.profiles || []).forEach(function(item) { states[item.id] = item; });
		}
		index(data[1] || {});
		function field(value, placeholder) {
			return E('input', {'class': 'cbi-input-text', value: value || '', placeholder: placeholder || ''});
		}
		function apply(section, f) {
			var id = section['.name'];
			if (!/^[a-z0-9_-]{1,32}$/.test(id) || !f.label.value.trim() || !f.device.value.trim() || !f.node.value.trim()) {
				note(wlocI18n.t('Save failed'), wlocI18n.t('Use a valid id, label, device address, and node.'), 'error');
				return;
			}
			uci.set('wloc-service', id, 'label', f.label.value.trim());
			uci.set('wloc-service', id, 'assigned_device', f.device.value.trim());
			uci.set('wloc-service', id, 'node_ref', f.node.value.trim());
			uci.set('wloc-service', id, 'node_mode', f.nodeMode.value);
			uci.set('wloc-service', id, 'geo_source', f.geo.value);
			uci.set('wloc-service', id, 'manual_lat', f.lat.value.trim());
			uci.set('wloc-service', id, 'manual_lon', f.lon.value.trim());
			uci.set('wloc-service', id, 'enabled', f.enabled.checked ? '1' : '0');
			uci.save('wloc-service').then(function() { return ui.changes.apply(true); })
				.then(function() { return restart(); }).then(function() {
					note(wlocI18n.t('Saved'), wlocI18n.t('Unified supervisor restarted.'));
					return uci.load('wloc-service');
				}).then(render).catch(function(error) {
					note(wlocI18n.t('Save failed'), String(error), 'error');
				});
		}
		function render() {
			body.innerHTML = '';
			var sections = uci.sections('wloc-service', 'device');
			if (!sections.length) {
				body.appendChild(E('tr', {}, [E('td', {'colspan': 8}, wlocI18n.t('No v2 device profiles yet.'))]));
				return;
			}
			sections.forEach(function(section) {
				var f = {label: field(section.label, 'label'), device: field(section.assigned_device, '192.168.1.100 or MAC'), node: field(section.node_ref || 'default', 'node'), lat: field(section.manual_lat, 'lat'), lon: field(section.manual_lon, 'lon'), enabled: E('input', {type: 'checkbox', checked: section.enabled === '1'})};
			f.nodeMode = E('select', {}); [['fixed','fixed'],['gateway_default','gateway_default']].forEach(function(v) { f.nodeMode.appendChild(E('option', {value: v[0], selected: (section.node_mode || 'fixed') === v[0]}, v[1])); });
			f.geo = E('select', {}); [['auto','auto'],['manual','manual']].forEach(function(v) { f.geo.appendChild(E('option', {value: v[0], selected: (section.geo_source || 'auto') === v[0]}, v[1])); });
			var state = states[section['.name']];
			body.appendChild(E('tr', {class: 'tr'}, [
				E('td', {class: 'td'}, section['.name']), E('td', {class: 'td'}, f.label), E('td', {class: 'td'}, f.device), E('td', {class: 'td'}, f.node),
				E('td', {class: 'td'}, [f.nodeMode, ' / ', f.geo]), E('td', {class: 'td'}, [f.lat, ' ', f.lon]),
				E('td', {class: 'td'}, [f.enabled, ' ', state ? state.phase : wlocI18n.t('Not observed')]),
				E('td', {class: 'td'}, E('button', {class: 'cbi-button cbi-button-apply', click: function() { apply(section, f); }}, wlocI18n.t('Save')))
			]));
			});
		}
		function add() {
			var id = window.prompt(wlocI18n.t('New profile id'), 'phone');
			if (!id || !/^[a-z0-9_-]{1,32}$/.test(id) || uci.get('wloc-service', id)) {
				note(wlocI18n.t('Add failed'), wlocI18n.t('Use a unique lowercase profile id.'), 'error');
				return;
			}
			uci.add('wloc-service', 'device', id);
			uci.set('wloc-service', id, 'label', id); uci.set('wloc-service', id, 'node_ref', 'default');
			uci.set('wloc-service', id, 'node_mode', 'fixed'); uci.set('wloc-service', id, 'geo_source', 'auto'); uci.set('wloc-service', id, 'enabled', '0');
			uci.save('wloc-service').then(function() { return ui.changes.apply(true); }).then(function() { return uci.load('wloc-service'); }).then(render);
		}
		render();
		poll.add(function() { return L.resolveDefault(health(), {}).then(function(value) { index(value || {}); render(); }); }, 5);
		return E([], [E('h2', {}, wlocI18n.t('Device profiles')), E('p', {}, wlocI18n.t('One profile owns one device binding, node selection, WLOC auto/manual mode, and isolated redirect.')), E('p', {}, E('button', {class: 'cbi-button cbi-button-add', click: add}, wlocI18n.t('Add profile'))), E('div', {class: 'cbi-section', style: 'overflow:auto'}, E('table', {class: 'table'}, [E('tr', {class: 'tr table-titles'}, ['ID', wlocI18n.t('Label'), wlocI18n.t('Device'), wlocI18n.t('Node'), wlocI18n.t('Mode'), wlocI18n.t('Manual location'), wlocI18n.t('Enabled / state'), ''].map(function(v) { return E('th', {class: 'th'}, v); })), body]))]);
	}
});
