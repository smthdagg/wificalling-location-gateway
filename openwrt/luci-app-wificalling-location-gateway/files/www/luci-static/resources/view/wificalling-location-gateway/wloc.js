'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require form';
'require fs';
'require poll';
'require uci';
'require ui';
'require rpc';

// WLOC Location module of the Wi-Fi Calling & WLOC gateway.
//
// Eight features on one page:
//   1. Safari CA profile (link + regenerate)
//   2. Location mode: auto (follow node) / manual
//   3. Module on/off switch
//   4. Current geo info (country/city, read-only)
//   5. Full GPS values (read-only, admin-only file)
//   6. Manual search (Nominatim) and coordinate entry
//   7. Saved presets, one-tap apply
//   8. WLOC usage log (events.jsonl)

var callCtl = rpc.declare({
	object: 'luci.wloc',
	method: 'ctl',
	params: [ 'method', 'query', 'lat', 'lon' ]
});

var regenProfile = rpc.declare({
	object: 'luci.wloc',
	method: 'regen_profile'
});

var certInfo = rpc.declare({
	object: 'luci.wloc',
	method: 'cert_info'
});

var regenCa = rpc.declare({
	object: 'luci.wloc',
	method: 'regen_ca'
});

var restartService = rpc.declare({
	object: 'luci.wloc',
	method: 'restart_service'
});

var verifyFingerprint = rpc.declare({
	object: 'luci.wloc',
	method: 'verify_fingerprint',
	params: [ 'fingerprint' ]
});

var STATUS_FILE = '/var/run/wloc-service/status.json';
var EVENTS_FILE = '/var/run/wloc-service/events.jsonl';
// The CA profile is served by this router's uhttpd; derive the address
// from the page the admin is using instead of a hardcoded subnet, so the
// link works on any LAN (e.g. 192.168.50.1).
var profileHost = (location.hostname.indexOf(':') >= 0)
	? '[' + location.hostname + ']' : location.hostname;
var PROFILE_URL = 'http://' + profileHost + '/wloc-ca.mobileconfig';
function fmtTime(unix) {
	if (!unix) return '-';
	return new Date(unix * 1000).toLocaleString();
}

function notify(title, message) {
	ui.addNotification(null, E('p', [ E('strong', title + ': '), message ]));
}

function gpsOf(geo) {
	return (geo && geo.latitude != null && geo.longitude != null)
		? geo.latitude.toFixed(6) + ' / ' + geo.longitude.toFixed(6) : '-';
}

return view.extend({
	load: function() {
		// Auto-repack the iOS CA profile on every page load (idempotent,
		// sub-100ms): the profile link must work even before anyone clicks
		// "Regenerate profile", otherwise a fresh install - or a secondary
		// router without DHCP - shows a dead /wloc-ca.mobileconfig link.
		var autoRegen = regenProfile().then(function(r) { return r; },
			function(e) { return { error: String(e) }; });
		return Promise.all([
			L.resolveDefault(fs.read(STATUS_FILE), '{}'),
			L.resolveDefault(fs.read(EVENTS_FILE), ''),
			uci.load('wloc-service'),
			uci.load('wificalling-gateway'),
			L.resolveDefault(certInfo(), {}),
			L.resolveDefault(fs.read('/var/run/wloc-service/proxy-health.json'), '{}'),
			autoRegen
		]);
	},

	render: function(data) {
		wlocI18n.localizeTabs();
		var status;
		try { status = JSON.parse(data[0]); } catch (e) { status = {}; }
		var eventsText = data[1] || '';
		var ca = data[4] || {};
		var proxyHealth;
		try { proxyHealth = JSON.parse(data[5] || '{}'); } catch (e) { proxyHealth = {}; }
		var regen = data[6] || {};
		var deviceList = uci.sections('wificalling-gateway', 'device').map(function(d) {
			return { ip: d.source_ip, label: d.label || d.source_ip };
		}).filter(function(d) { return d.ip; });

		var m = new form.Map('wloc-service', wlocI18n.t('WLOC Settings'),
			wlocI18n.t('WLOC location interception: spoofs the Apple WLOC response so the test device reports the gateway-chosen location. GPS values stay on this router.'));

		/* ---------- 3. Module on/off switch ---------- */
		var so = m.section(form.NamedSection, 'main', 'wloc-service');
		so.anonymous = true;
		so.title = wlocI18n.t('Module');

		var enabled = so.option(form.Flag, 'enabled', wlocI18n.t('Enable WLOC interception'),
			wlocI18n.t('Turns the WLOC rewrite on/off. The nftables redirect stays in place; while off, Apple WLOC traffic passes through untouched.'));
		enabled.onchange = function(ev, section_id, value) {
			var on = (value === true || value === 1 || value === '1');
			callCtl(on ? 'enable' : 'disable', null, null, null).then(function(r) {
				if (r.error) {
					notify(wlocI18n.t('Switch failed'), r.error);
					return;
				}
				uci.set('wloc-service', 'main', 'enabled', on ? '1' : '0');
				uci.save('wloc-service');
				ui.changes.apply(true);
			});
		};

		so.option(form.DummyValue, '_phase', wlocI18n.t('Service phase')).cfgvalue = function() {
			return status.service_phase || '-';
		};

		/* ---------- 2. Location mode: auto / manual ---------- */
		var mode = so.option(form.ListValue, 'geo_source', wlocI18n.t('Location mode'),
			wlocI18n.t('Auto follows the sing-box node bound to the test device. Manual uses the search result or coordinates below.'));
		mode.value('auto', wlocI18n.t('Auto (follow node)'));
		mode.value('manual', wlocI18n.t('Manual location'));
		mode.onchange = function(ev, section_id, value) {
			var main = uci.get('wloc-service', 'main');
			var manualLat = main && main.manual_lat;
			var manualLon = main && main.manual_lon;
			if (value === 'manual' && (!manualLat || !manualLon)) {
				notify(wlocI18n.t('Mode switch failed'),
					wlocI18n.t('Enter and apply manual coordinates first.'));
				return Promise.resolve(false);
			}
			return callCtl('mode-set', value,
				value === 'manual' ? manualLat : null,
				value === 'manual' ? manualLon : null).then(function(response) {
				if (response && response.error) {
					notify(wlocI18n.t('Mode switch failed'), response.error);
					return false;
				}
				uci.set('wloc-service', 'main', 'geo_source', value);
				return true;
			}).catch(function(e) {
				notify(wlocI18n.t('Mode switch failed'), String(e));
				return false;
			});
		};

		so.option(form.Value, 'manual_lat', wlocI18n.t('Manual latitude'));
		so.option(form.Value, 'manual_lon', wlocI18n.t('Manual longitude'));

		var follow = so.option(form.ListValue, 'assigned_device', wlocI18n.t('Follow device'),
			wlocI18n.t('The device whose bound node the WLOC location follows (its exit IP drives auto mode).'));
		deviceList.forEach(function(d) {
			follow.value(d.ip, d.label + ' (' + d.ip + ')');
		});
		follow.onchange = function(ev, section_id, value) {
			uci.set('wloc-service', 'main', 'assigned_device', value);
			uci.save('wloc-service').then(function() {
				return ui.changes.apply(true);
			}).then(function() {
				return restartService();
			}).then(function(r) {
				if (r && r.error)
					notify(wlocI18n.t('Apply failed'), r.error);
				else
					notify(wlocI18n.t('Applied'), wlocI18n.t('Device saved. WLOC now follows its node.'));
			}).catch(function(e) {
				notify(wlocI18n.t('Apply failed'), String(e));
			});
		};

		/* ---------- 6. Manual search + coordinate apply ---------- */
		var searchResult = E('div', { 'class': 'cbi-row', 'id': 'wloc-search-result' });
		var queryField = E('input', {
			'class': 'cbi-input-text',
			'id': 'wloc-search-query',
			'type': 'text',
			'placeholder': wlocI18n.t('e.g. London, UK')
		});
		var searchBtn = E('button', {
			'class': 'cbi-button cbi-button-apply',
			'id': 'wloc-search-btn',
			'click': function() {
				var q = document.getElementById('wloc-search-query').value.trim();
				if (!q) return;
				this.disabled = true;
				// 先搜索：只返回城市与坐标，不改变当前定位。
				callCtl('geo-search', q, null, null).then(function(r) {
					searchBtn.disabled = false;
					var found = r && r.result;
					if (r.error || !found || found.latitude == null || found.longitude == null) {
						notify(wlocI18n.t('Search failed'), r.error || wlocI18n.t('Place not found'));
						return;
					}
					var city = found.city || q;
					var lat = String(Number(found.latitude).toFixed(6));
					var lon = String(Number(found.longitude).toFixed(6));
					document.getElementById('wloc-coord-lat').value = lat;
					document.getElementById('wloc-coord-lon').value = lon;
					searchResult.innerHTML = '';
					searchResult.appendChild(E('p', {}, wlocI18n.t('Search result: ') + city +
						wlocI18n.t(' (lat ') + lat + wlocI18n.t(', lon ') + lon + wlocI18n.t(')') +
						wlocI18n.t(') - click "Apply coordinates" to activate.')));
				});
			}
		}, wlocI18n.t('Search'));

		var coordLat = E('input', { 'class': 'cbi-input-text', 'id': 'wloc-coord-lat', 'type': 'text', 'placeholder': '51.5074' });
		var coordLon = E('input', { 'class': 'cbi-input-text', 'id': 'wloc-coord-lon', 'type': 'text', 'placeholder': '-0.1278' });
		var coordBtn = E('button', {
			'class': 'cbi-button cbi-button-apply',
			'id': 'wloc-coord-btn',
			'click': function() {
				var lat = document.getElementById('wloc-coord-lat').value.trim();
				var lon = document.getElementById('wloc-coord-lon').value.trim();
				if (!lat || !lon) return;
				this.disabled = true;
				callCtl('geo-set', null, lat, lon).then(function(r) {
					coordBtn.disabled = false;
					if (r.error) {
						notify(wlocI18n.t('Apply failed'), r.error);
						return;
					}
					uci.set('wloc-service', 'main', 'manual_lat', lat);
					uci.set('wloc-service', 'main', 'manual_lon', lon);
					uci.set('wloc-service', 'main', 'geo_source', 'manual');
					uci.save('wloc-service');
					ui.changes.apply(true);
					notify(wlocI18n.t('Applied'), wlocI18n.t('Coordinates are now the active location.'));
				});
			}
		}, wlocI18n.t('Apply coordinates'));

		var searchBox = E('div', { 'class': 'cbi-section' }, [
			E('h3', {}, wlocI18n.t('Manual search')),
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' }, [
					E('label', { 'for': 'wloc-search-query', 'class': 'cbi-value-title' },
						[ wlocI18n.t('Place name'), ' ', wlocI18n.t('(online search)') ]),
					E('div', { 'class': 'cbi-value-field' }, [ queryField, ' ', searchBtn ])
				])),
			searchResult,
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' }, [
					E('label', { 'class': 'cbi-value-title' }, wlocI18n.t('Or enter coordinates')),
					E('div', { 'class': 'cbi-value-field' }, [ coordLat, ' ', coordLon, ' ', coordBtn ])
				]))
		]);

		/* ---------- 7. Presets (self-drawn table) ---------- */
		var presetBody = E('tbody', { 'id': 'wloc-preset-body' });
		var presetTable = E('table', { 'class': 'cbi-section-table' }, [
			E('thead', {},
				E('tr', {}, [
					E('th', {}, wlocI18n.t('Label')),
					E('th', {}, wlocI18n.t('Latitude')),
					E('th', {}, wlocI18n.t('Longitude')),
					E('th', {}, '')
				])),
			presetBody
		]);

		function applyPreset(sid) {
			var s = uci.get('wloc-service', sid);
			if (!s || !s.latitude || !s.longitude) {
				notify(wlocI18n.t('Apply failed'), wlocI18n.t('Preset has no coordinates.'));
				return;
			}
			uci.set('wloc-service', 'main', 'manual_lat', s.latitude);
			uci.set('wloc-service', 'main', 'manual_lon', s.longitude);
			uci.set('wloc-service', 'main', 'geo_source', 'manual');
			uci.save('wloc-service').then(function() {
				return ui.changes.apply(true);
			}).then(function() {
				return callCtl('geo-set', null, s.latitude, s.longitude);
			}).then(function(r) {
				if (r && r.error) notify(wlocI18n.t('Apply failed'), r.error);
				else notify(wlocI18n.t('Applied'), wlocI18n.t('Preset is now the active location.'));
			}).catch(function(e) {
				notify(wlocI18n.t('Apply failed'), String(e));
			});
		}

		function removePreset(sid) {
			uci.delete('wloc-service', sid);
			uci.save('wloc-service').then(function() {
				return ui.changes.apply(true);
			}).then(function() {
				renderPresets();
			}).catch(function(e) {
				notify(wlocI18n.t('Apply failed'), String(e));
			});
		}

		function renderPresets() {
			presetBody.innerHTML = '';
			var presets = uci.sections('wloc-service', 'preset');
			if (!presets.length) {
				presetBody.appendChild(E('tr', {}, [ E('td', { 'colspan': 4 }, wlocI18n.t('No saved locations yet.')) ]));
				return;
			}
			presets.forEach(function(s) {
				var sid = s['.name'];
				presetBody.appendChild(E('tr', {}, [
					E('td', {}, s.label || sid),
					E('td', {}, s.latitude || '-'),
					E('td', {}, s.longitude || '-'),
					E('td', {}, [
						E('button', {
							'class': 'cbi-button cbi-button-apply',
							'click': function() { applyPreset(sid); }
						}, wlocI18n.t('Apply')),
						' ',
						E('button', {
							'class': 'cbi-button cbi-button-remove',
							'click': function() { removePreset(sid); }
						}, wlocI18n.t('Delete'))
					])
				]));
			});
		}
		renderPresets();

		var addPresetBtn = E('button', {
			'class': 'cbi-button cbi-button-add',
			'click': function() {
				var labelInput = E('input', { 'class': 'cbi-input-text', 'placeholder': wlocI18n.t('Label') });
				var latInput = E('input', { 'class': 'cbi-input-text', 'placeholder': '51.5074' });
				var lonInput = E('input', { 'class': 'cbi-input-text', 'placeholder': '-0.1278' });
				ui.showModal(wlocI18n.t('Add saved location'), [
					E('p', {}, [
						labelInput, ' ', latInput, ' ', lonInput
					]),
					E('div', { 'class': 'right' }, [
						E('button', { 'class': 'btn', 'click': ui.hideModal }, _('Cancel')),
						' ',
						E('button', { 'class': 'btn cbi-button-positive', 'click': function() {
							var label = labelInput.value.trim();
							var lat = latInput.value.trim();
							var lon = lonInput.value.trim();
							if (!label || !lat || !lon) return;
							var sid = uci.add('wloc-service', 'preset');
							uci.set('wloc-service', sid, 'label', label);
							uci.set('wloc-service', sid, 'latitude', lat);
							uci.set('wloc-service', sid, 'longitude', lon);
							uci.save('wloc-service').then(function() {
								return ui.changes.apply(true);
							}).then(function() {
								ui.hideModal();
								renderPresets();
								notify(wlocI18n.t('Applied'), wlocI18n.t('Preset saved.'));
							}).catch(function(e) {
								notify(wlocI18n.t('Apply failed'), String(e));
							});
						} }, _('Save'))
					])
				]);
			}
		}, wlocI18n.t('Add saved location'));

		var presetsBox = E('div', { 'class': 'cbi-section' }, [
			E('h3', {}, wlocI18n.t('Saved locations')),
			presetTable,
			E('p', {}, addPresetBtn)
		]);

		/* ---------- 1. Safari certificate ---------- */
		var certLink;
		if (regen && regen.error) {
			// The profile could not be (re)generated on this router: show
			// why instead of a link that would 403/404 in the browser.
			certLink = E('span', { 'class': 'alert-message warning', 'id': 'wloc-cert-link' },
				wlocI18n.t('Profile unavailable: ') + String(regen.error));
		} else {
			certLink = E('a', { 'href': PROFILE_URL, 'target': '_blank', 'id': 'wloc-cert-link' }, PROFILE_URL);
		}
		function fmtCertTime(unix) {
			return unix ? new Date(unix * 1000).toLocaleString() : wlocI18n.t('Unknown');
		}

		var certInfoBody = E('tbody', { 'id': 'wloc-cert-info' });
		var certInfoTable = E('table', { 'class': 'cbi-section-table' }, certInfoBody);

		function renderCertInfo(info) {
			certInfoBody.innerHTML = '';
			if (!info || info.error || !info.fingerprint) {
				certInfoBody.appendChild(E('tr', {}, [ E('td', {}, wlocI18n.t('CA info missing - start the service first.')) ]));
				return;
			}
			var rows = [
				[wlocI18n.t('Fingerprint'), info.fingerprint],
				[wlocI18n.t('Issued at'), fmtCertTime(info.issued_at)],
				[wlocI18n.t('Expires at'), fmtCertTime(info.expires_at)]
			];
			rows.forEach(function(r) {
				certInfoBody.appendChild(E('tr', {}, [
					E('td', { 'class': 'td' }, r[0]),
					E('td', { 'class': 'td' }, r[1])
				]));
			});
		}
		renderCertInfo(ca);

		var certText = E('div', { 'class': 'cbi-value' }, [
			E('label', { 'class': 'cbi-value-title' }, wlocI18n.t('Profile link')),
			E('div', { 'class': 'cbi-value-field' }, certLink)
		]);

		// Repack the iOS profile with the CURRENT CA - the root certificate
		// stays unchanged, so devices that already trust it keep working.
		var repackBtn = E('button', {
			'class': 'cbi-button cbi-button-apply',
			'id': 'wloc-repack-btn',
			'click': function() {
				this.disabled = true;
				regenProfile().then(function(r) {
					repackBtn.disabled = false;
					if (r.error) {
						notify(wlocI18n.t('Regenerate failed'), r.error);
						return;
					}
					notify(wlocI18n.t('Profile ready'),
						wlocI18n.t('On the iPhone open Safari and visit %s, then enable full trust in Settings > General > About > Certificate Trust Settings.')
							.format(r.url || PROFILE_URL));
				}).catch(function(e) {
					repackBtn.disabled = false;
					notify(wlocI18n.t('Regenerate failed'), String(e));
				});
			}
		}, wlocI18n.t('Regenerate profile'));

		// Generate a brand-new root CA: the fingerprint changes and EVERY
		// device must reinstall and re-trust the profile.
		var newCaBtn = E('button', {
			'class': 'cbi-button cbi-button-remove',
			'id': 'wloc-newca-btn',
			'click': function() {
				ui.showModal(wlocI18n.t('Generate a new root certificate?'), [
					E('p', {}, wlocI18n.t('This replaces the root CA. All devices must reinstall the profile and enable full trust again.')),
					E('div', { 'class': 'right' }, [
						E('button', { 'class': 'btn', 'click': ui.hideModal }, wlocI18n.t('Cancel')),
						' ',
						E('button', { 'class': 'btn cbi-button-negative', 'click': function() {
							ui.hideModal();
							newCaBtn.disabled = true;
							regenCa().then(function(r) {
								newCaBtn.disabled = false;
								if (r.error) {
									notify(wlocI18n.t('Regenerate failed'), r.error);
									return;
								}
								renderCertInfo(r);
								notify(wlocI18n.t('Profile ready'),
									wlocI18n.t('New CA generated. Reinstall and trust it on the iPhone.'));
							}).catch(function(e) {
								newCaBtn.disabled = false;
								notify(wlocI18n.t('Regenerate failed'), String(e));
							});
						} }, wlocI18n.t('Generate new CA'))
					])
				]);
			}
		}, wlocI18n.t('Generate new CA'));

		function fmtHealth(t) {
			return t ? new Date(t * 1000).toLocaleString() : wlocI18n.t('No handshakes yet.');
		}
		var trustText = proxyHealth.last_failure && (!proxyHealth.last_success || proxyHealth.last_failure > proxyHealth.last_success)
			? wlocI18n.t('Handshake failed - the iPhone does not trust this CA. Reinstall the profile and enable full trust.')
			: wlocI18n.t('Handshake ok');
		var trustRow = E('div', { 'class': 'cbi-value' }, [
			E('label', { 'class': 'cbi-value-title' }, wlocI18n.t('Certificate trust')),
			E('div', { 'class': 'cbi-value-field' }, E('span', { 'class': proxyHealth.last_failure ? 'alert-message warning' : '' }, trustText))
		]);

		var fpInput = E('input', { 'class': 'cbi-input-text', 'id': 'wloc-fp-input', 'type': 'text', 'placeholder': 'CD:65:EC:B5:...' });
		var fpResult = E('span', { 'id': 'wloc-fp-result' });
		var verifyBtn = E('button', {
			'class': 'cbi-button cbi-button-apply',
			'click': function() {
				var fp = document.getElementById('wloc-fp-input').value.trim();
				if (!fp) return;
				this.disabled = true;
				verifyFingerprint(fp).then(function(r) {
					verifyBtn.disabled = false;
					var out;
					if (r.error)
						out = E('span', { 'class': 'alert-message error' }, r.error);
					else if (r.match)
						out = E('span', { 'class': 'alert-message' }, wlocI18n.t('Match - the iPhone trusts this CA.'));
					else
						out = E('span', { 'class': 'alert-message warning' }, wlocI18n.t('Mismatch - reinstall the profile on the iPhone and enable full trust.'));
					fpResult.innerHTML = '';
					fpResult.appendChild(out);
				}).catch(function(e) {
					verifyBtn.disabled = false;
					fpResult.innerHTML = '';
					fpResult.appendChild(E('span', { 'class': 'alert-message error' }, String(e)));
				});
			}
		}, wlocI18n.t('Verify'));

		var verifyRow = E('div', { 'class': 'cbi-value' }, [
			E('label', { 'class': 'cbi-value-title' }, wlocI18n.t('Verify iPhone certificate')),
			E('div', { 'class': 'cbi-value-field' }, [
				fpInput, ' ', verifyBtn, ' ',
				E('p', {}, wlocI18n.t('Paste the fingerprint shown on the iPhone (Settings > General > VPN & Device Management > wloc-service profile).')),
				fpResult
			])
		]);

		var certBox = E('div', { 'class': 'cbi-section' }, [
			E('h3', {}, wlocI18n.t('Certificate (Safari install)')),
			E('h4', {}, wlocI18n.t('CA info')),
			certInfoTable,
			trustRow,
			verifyRow,
			certText,
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' }, [
					E('label', { 'class': 'cbi-value-title' }, wlocI18n.t('Install steps')),
					E('div', { 'class': 'cbi-value-field' },
						wlocI18n.t('1. Open the profile link in Safari on the test iPhone. ') +
						wlocI18n.t('2. Install the configuration profile. ') +
						wlocI18n.t('3. Enable full trust in Settings > General > About > Certificate Trust Settings.'))
				])),
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' }, [
					E('label', { 'class': 'cbi-value-title' }, ''),
					E('div', { 'class': 'cbi-value-field' }, [ repackBtn, ' ', newCaBtn ])
				]))
		]);

		return m.render().then(function(formNode) {
			return E([], [formNode, searchBox, presetsBox, certBox]);
		});
	}
});
