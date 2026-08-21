'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require poll';
'require rpc';
'require ui';

var getHealth = rpc.declare({ object: 'luci.wloc', method: 'health' });
var restartService = rpc.declare({ object: 'luci.wloc', method: 'restart_unified' });
var createSupportBundle = rpc.declare({ object: 'luci.wloc', method: 'support_bundle' });

function notify(title, message, kind) {
	ui.addNotification(null, E('p', [ E('strong', title + ': '), message ]), kind);
}
function dot(ok, text) {
	return E('span', {}, [
		E('span', { style: 'display:inline-block;width:8px;height:8px;border-radius:50%;' +
			(ok ? 'background:#16a34a;' : 'background:#dc2626;') + 'margin-right:6px' }),
		text
	]);
}
function yesNo(value) {
	return dot(!!value, value ? wlocI18n.t('Yes') : wlocI18n.t('No'));
}
function renderRows(body, rows) {
	body.innerHTML = '';
	rows.forEach(function(row) {
		body.appendChild(E('div', { style: 'padding:3px 0' }, [
			E('span', { style: 'display:inline-block;width:132px;color:#666' }, row[0]), row[1]
		]));
	});
}
function renderProfiles(body, profiles) {
	body.innerHTML = '';
	(profiles || []).forEach(function(profile) {
		var good = profile.phase === 'intercepting' || profile.phase === 'disabled';
		body.appendChild(E('tr', { class: 'tr' }, [
			E('td', { class: 'td' }, profile.id || '-'),
			E('td', { class: 'td' }, profile.label || '-'),
			E('td', { class: 'td' }, dot(good, profile.phase || '-')),
			E('td', { class: 'td' }, profile.reason_code || '-')
		]));
	});
}

return view.extend({
	load: function() {
		return L.resolveDefault(getHealth(), { error: wlocI18n.t('Health check unavailable') });
	},
	render: function(data) {
		wlocI18n.localizeTabs();
		var health = data || {};
		var wlocBody = E('div', {}, []);
		var providerBody = E('div', {}, []);
		var redirectBody = E('div', {}, []);
		var profileBody = E('tbody', {}, []);

		function renderHealth(h) {
			var services = h.services || {};
			var wloc = services.wloc || {};
			var provider = services.provider || {};
			var redirect = services.redirect || {};
			renderRows(wlocBody, [
				[wlocI18n.t('Daemon'), dot(!!wloc.running, wloc.running ? wlocI18n.t('Running') : wlocI18n.t('Stopped'))],
				[wlocI18n.t('Control socket'), yesNo(wloc.socket)],
				[wlocI18n.t('Status file'), dot(!!wloc.status_fresh, wloc.status_fresh ? wlocI18n.t('Fresh') : wlocI18n.t('Stale'))],
				[wlocI18n.t('Phase'), wloc.phase || '-'],
				[wlocI18n.t('Exit probe'), dot(wloc.exit === 'verified', wloc.exit || '-')],
				[wlocI18n.t('Geo'), dot(wloc.geo === 'fresh', wloc.geo || '-')],
				[wlocI18n.t('Last error'), wloc.last_error || '-']
			]);
			renderRows(providerBody, [
				[wlocI18n.t('Provider'), dot(!!provider.available, provider.available ? wlocI18n.t('Available') : wlocI18n.t('Unavailable'))],
				[wlocI18n.t('Executable'), provider.path || '-'],
				[wlocI18n.t('Config present'), yesNo(provider.config_present)],
				[wlocI18n.t('Config valid'), dot(!!provider.config_valid, provider.config_valid ? wlocI18n.t('Valid') : wlocI18n.t('Invalid'))],
				[wlocI18n.t('Config age'), provider.config_age || '-'],
				[wlocI18n.t('Provider error'), provider.last_error || '-']
			]);
			renderRows(redirectBody, [
				[wlocI18n.t('Ruleset'), dot(!!redirect.table_present, redirect.table_present ? wlocI18n.t('Present') : wlocI18n.t('Missing'))],
				[wlocI18n.t('Rules'), String(redirect.rules || 0)],
				[wlocI18n.t('Profiles'), String((h.profiles || []).length)]
			]);
			renderProfiles(profileBody, h.profiles);
		}
		renderHealth(health);
		poll.add(function() {
			return L.resolveDefault(getHealth(), { error: wlocI18n.t('Health check unavailable') }).then(renderHealth);
		}, 10);

		var restart = E('button', { class: 'cbi-button cbi-button-apply', click: function() {
			restart.disabled = true;
			restartService().then(function(result) {
				if (result && result.error) throw new Error(result.error);
				notify(wlocI18n.t('Service restarted'), wlocI18n.t('WLOC service lifecycle restarted.'), 'info');
				return getHealth().then(renderHealth);
			}).catch(function(error) {
				notify(wlocI18n.t('Restart failed'), String(error), 'error');
			}).then(function() { restart.disabled = false; });
		} }, wlocI18n.t('Restart WLOC service'));

		var bundle = E('button', { class: 'cbi-button', click: function() {
			bundle.disabled = true;
			createSupportBundle().then(function(result) {
				if (result && result.error) throw new Error(result.error);
				notify(wlocI18n.t('Support bundle ready'),
					(result && result.path) || '/tmp/wloc-support-bundle.tar.gz', 'info');
			}).catch(function(error) {
				notify(wlocI18n.t('Support bundle failed'), String(error), 'error');
			}).then(function() { bundle.disabled = false; });
		} }, wlocI18n.t('Generate support bundle'));

		return E([], [
			E('h2', {}, wlocI18n.t('WLOC Service Status')),
			E('p', {}, wlocI18n.t('This page reports the standalone WLOC lifecycle, provider, redirect rules, and device profiles. Component updates are managed on the separate Component Update page.')),
			E('div', { class: 'cbi-section' }, [E('h3', {}, wlocI18n.t('WLOC service')), wlocBody]),
			E('div', { class: 'cbi-section' }, [E('h3', {}, wlocI18n.t('Provider')), providerBody]),
			E('div', { class: 'cbi-section' }, [E('h3', {}, wlocI18n.t('Redirect')), redirectBody]),
			E('div', { class: 'cbi-section' }, [E('h3', {}, wlocI18n.t('Device profiles')), E('table', { class: 'table' }, [
				E('tr', { class: 'tr table-titles' }, ['ID', 'Label', 'State', 'Reason'].map(function(value) {
					return E('th', { class: 'th' }, wlocI18n.t(value));
				})), profileBody
			])]),
			E('div', { class: 'cbi-section' }, [restart, ' ', bundle])
		]);
	}
});

