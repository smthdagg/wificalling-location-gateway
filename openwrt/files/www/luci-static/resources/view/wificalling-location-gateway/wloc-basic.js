'use strict';
'require view';
'require form';
'require uci';
'require rpc';
'require poll';
'require ui';
'require wificalling-location-gateway.i18n as i18n';

var getHealth = rpc.declare({ object: 'luci.wloc', method: 'health' });
var certInfo = rpc.declare({ object: 'luci.wloc', method: 'cert_info' });
var regenProfile = rpc.declare({ object: 'luci.wloc', method: 'regen_profile' });
var regenCa = rpc.declare({ object: 'luci.wloc', method: 'regen_ca' });
var verifyFingerprint = rpc.declare({ object: 'luci.wloc', method: 'verify_fingerprint', params: ['fingerprint'] });

function timeLabel(value) {
	return value ? new Date(Number(value) * 1000).toLocaleString() : '-';
}

function statusDot(ok) {
	return E('span', { style: 'display:inline-block;width:8px;height:8px;border-radius:50%;background:' + (ok ? '#16a34a' : '#dc2626') + ';margin-right:6px' });
}

return view.extend({
	load: function() {
		return Promise.all([
			uci.load('wloc-service'),
			L.resolveDefault(getHealth(), {}),
			L.resolveDefault(certInfo(), {})
		]);
	},

	render: function(data) {
		i18n.localizeTabs();
		var health = data[1] || {};
		var certificate = data[2] || {};
		var profileUrl = 'http://' + window.location.hostname + '/wloc-ca.mobileconfig';
		var healthBody = E('div', {});
		var certificateBody = E('tbody', {});
		var verifyResult = E('span', { style: 'margin-left:8px' });

		function renderHealth(current) {
			var services = (current || {}).services || {};
			var gateway = services.gateway || {};
			var wloc = services.wloc || {};
			var provider = services.provider || {};
			var redirect = services.redirect || {};
			healthBody.innerHTML = '';
			[
				[i18n.t('WiFi Calling Gateway'), gateway.running ? i18n.t('Running') : i18n.t('Stopped'), gateway.enabled],
				[i18n.t('WLOC daemon'), wloc.running ? i18n.t('Running') : i18n.t('Stopped'), wloc.running],
				[i18n.t('Provider'), provider.available ? i18n.t('Available') : i18n.t('Unavailable'), provider.available],
				[i18n.t('Redirect table'), redirect.table_present ? i18n.t('Present') : i18n.t('Absent'), redirect.table_present],
				[i18n.t('Phase'), wloc.phase || i18n.t('Unknown'), true]
			].forEach(function(row) {
				healthBody.appendChild(E('div', { class: 'cbi-value' }, [
					E('label', { class: 'cbi-value-title' }, row[0]),
					E('div', { class: 'cbi-value-field' }, [statusDot(row[2]), row[1]])
				]));
			});
		}

		function renderCertificate(info) {
			certificate = info || {};
			certificateBody.innerHTML = '';
			[
				[i18n.t('Certificate version'), i18n.t('WLOC root CA') + ' / SHA-256'],
				[i18n.t('Fingerprint'), certificate.fingerprint || i18n.t('Unknown')],
				[i18n.t('Issued at'), timeLabel(certificate.issued_at)],
				[i18n.t('Expires at'), timeLabel(certificate.expires_at)],
				[i18n.t('Profile version'), i18n.t('Apple configuration profile') + ' / PayloadVersion 1'],
				[i18n.t('Profile link'), E('a', { href: profileUrl, target: '_blank', rel: 'noopener' }, profileUrl)]
			].forEach(function(row) {
				certificateBody.appendChild(E('tr', { class: 'tr' }, [
					E('td', { class: 'td' }, row[0]), E('td', { class: 'td' }, row[1])
				]));
			});
		}

		renderHealth(health);
		renderCertificate(certificate);
		poll.add(function() {
			return Promise.all([
				L.resolveDefault(getHealth(), {}),
				L.resolveDefault(certInfo(), {})
			]).then(function(values) {
				renderHealth(values[0]);
				renderCertificate(values[1]);
			});
		}, 10);

		var m = new form.Map('wloc-service', i18n.t('WLOC Setting'),
			i18n.t('Configure WLOC and view the integrated Gateway/WLOC status from one page. Device-specific node and location settings are managed on WLOC Devices.'));
		var s = m.section(form.NamedSection, 'main', 'wloc-service');
		s.anonymous = true;
		s.option(form.Flag, 'enabled', i18n.t('Enable WLOC service'));
		var provider = s.option(form.ListValue, 'geo_provider', i18n.t('Geo provider'));
		provider.value('http', i18n.t('HTTP provider'));
		provider.value('stub', i18n.t('Stub provider'));
		var interval = s.option(form.Value, 'probe_interval', i18n.t('Probe interval (seconds)'));
		interval.datatype = 'range(30,86400)';
		interval.default = '300';
		var port = s.option(form.Value, 'probe_port', i18n.t('Probe port'));
		port.datatype = 'port';
		var config = s.option(form.Value, 'singbox_config', i18n.t('sing-box provider configuration'));
		config.description = i18n.t('Optional path to an existing provider configuration. This project does not manage another application configuration.');

		var link = E('a', { href: profileUrl, target: '_blank', rel: 'noopener' }, profileUrl);
		var regenerate = E('button', { class: 'cbi-button', click: function() {
			regenerate.disabled = true;
			regenProfile().then(function(result) {
				if (result && result.error) throw new Error(result.error);
				if (result && result.url) {
					profileUrl = result.url;
					link.href = result.url;
					link.textContent = result.url;
					renderCertificate(certificate);
				}
				ui.addNotification(null, E('p', {}, i18n.t('Profile ready')), 'info');
			}).catch(function(error) {
				ui.addNotification(null, E('p', {}, i18n.t('Regenerate failed') + ': ' + error), 'error');
			}).then(function() { regenerate.disabled = false; });
		} }, i18n.t('Regenerate profile'));

		var regenerateCa = E('button', { class: 'cbi-button cbi-button-negative', click: function() {
			if (!window.confirm(i18n.t('Generate a new root certificate?'))) return;
			regenerateCa.disabled = true;
			regenCa().then(function(result) {
				if (result && result.error) throw new Error(result.error);
				renderCertificate(result);
				ui.addNotification(null, E('p', {}, i18n.t('New CA generated. Reinstall and trust it on the iPhone.')), 'warning');
			}).catch(function(error) {
				ui.addNotification(null, E('p', {}, i18n.t('Regenerate failed') + ': ' + error), 'error');
			}).then(function() { regenerateCa.disabled = false; });
		} }, i18n.t('Generate new CA'));

		var fingerprintInput = E('input', { class: 'cbi-input-text', style: 'width: min(100%, 520px)', placeholder: 'AA:BB:CC:…' });
		var verify = E('button', { class: 'cbi-button', click: function() {
			verify.disabled = true;
			verifyFingerprint(fingerprintInput.value).then(function(result) {
				if (result && result.error) throw new Error(result.error);
				verifyResult.textContent = result.match ? i18n.t('Match - the iPhone trusts this CA.') : i18n.t('Mismatch - reinstall the profile on the iPhone and enable full trust.');
				verifyResult.style.color = result.match ? '#16a34a' : '#dc2626';
			}).catch(function(error) {
				verifyResult.textContent = String(error);
				verifyResult.style.color = '#dc2626';
			}).then(function() { verify.disabled = false; });
		} }, i18n.t('Verify'));

		return m.render().then(function(node) {
			return E([], [
				E('h2', {}, i18n.t('WLOC Setting')),
				E('p', {}, i18n.t('WLOC settings and the quick integrated service overview are combined here.')),
				E('div', { class: 'cbi-section' }, [E('h3', {}, i18n.t('Current status')), healthBody]),
				node,
				E('div', { class: 'cbi-section' }, [
					E('h3', {}, i18n.t('Certificate (Safari install)')),
					E('p', {}, i18n.t('Install the WLOC CA only on the authorized test device. The values below identify the exact CA and profile version currently served by this router.')),
					E('table', { class: 'table' }, [certificateBody]),
					E('h4', {}, i18n.t('Install steps')),
					E('ol', {}, [
						E('li', {}, i18n.t('1. Open the profile link in Safari on the test iPhone. ')),
						E('li', {}, i18n.t('2. Install the configuration profile. ')),
						E('li', {}, i18n.t('3. Enable full trust in Settings > General > About > Certificate Trust Settings.'))
					]),
					E('p', {}, [regenerate, ' ', regenerateCa]),
					E('p', {}, [i18n.t('Verify iPhone certificate'), ': ', fingerprintInput, ' ', verify, verifyResult]),
					E('p', { class: 'alert-message warning' }, i18n.t('This replaces the root CA. All devices must reinstall the profile and enable full trust again.'))
				])
			]);
		});
	}
});
