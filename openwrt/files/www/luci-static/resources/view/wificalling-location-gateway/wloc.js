'use strict';
'require view';
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
	params: [ 'method', 'query', 'lat', 'lon' ],
	expect: { result: true }
});

var regenProfile = rpc.declare({
	object: 'luci.wloc',
	method: 'regen_profile',
	expect: { result: true }
});

var STATUS_FILE = '/var/run/wloc-service/status.json';
var EVENTS_FILE = '/var/run/wloc-service/events.jsonl';
var PROFILE_URL = 'http://192.168.31.1/wloc-ca.mobileconfig';

function fmtTime(unix) {
	if (!unix) return '-';
	return new Date(unix * 1000).toLocaleString();
}

function notify(title, message) {
	ui.addNotification(null, E('p', E('strong', title + ': '), message));
}

function gpsOf(geo) {
	return (geo && geo.latitude != null && geo.longitude != null)
		? geo.latitude.toFixed(6) + ' / ' + geo.longitude.toFixed(6) : '-';
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
		var status;
		try { status = JSON.parse(data[0]); } catch (e) { status = {}; }
		var eventsText = data[1] || '';

		var map = new form.Map('wloc-service', _('Wi-Fi Calling & WLOC Gateway'),
			_('WLOC location interception: spoofs the Apple WLOC response so the test device ' +
			  'reports the gateway-chosen location. GPS values stay on this router.'));
		map.render();

		/* ---------- 3. Module on/off switch ---------- */
		var so = map.section(form.NamedSection, 'main', 'wloc-service');
		so.anonymous = true;
		so.title = _('Module');

		var enabled = so.option(form.Flag, 'enabled', _('Enable WLOC interception'),
			_('Turns the WLOC rewrite on/off. The nftables redirect stays in place; ' +
			  'while off, Apple WLOC traffic passes through untouched.'));
		enabled.onchange = function(section_id, value) {
			var on = (value === true || value === 1 || value === '1');
			callCtl(on ? 'enable' : 'disable', null, null, null).then(function(r) {
				if (r.error) {
					notify(_('Switch failed'), r.error);
					return;
				}
				uci.set('wloc-service', 'main', 'enabled', on ? '1' : '0');
				uci.save('wloc-service');
				uci.commit('wloc-service');
			});
		};

		so.option(form.DummyValue, '_phase', _('Service phase')).value = status.service_phase || '-';

		/* ---------- 2. Location mode: auto / manual ---------- */
		var mode = so.option(form.ListValue, 'geo_source', _('Location mode'),
			_('Auto follows the sing-box node bound to the test device. Manual uses the ' +
			  'search result or coordinates below.'));
		mode.value('auto', _('Auto (follow node)'));
		mode.value('manual', _('Manual location'));
		mode.onchange = function(section_id, value) {
			var main = uci.get('wloc-service', 'main');
			uci.set('wloc-service', 'main', 'geo_source', value);
			uci.save('wloc-service');
			uci.commit('wloc-service');
			if (value === 'auto') {
				callCtl('geo-clear', null, null, null).then(function(r) {
					if (r.error) notify(_('Mode switch failed'), r.error);
				});
			}
			else if (main && main.manual_lat && main.manual_lon) {
				callCtl('geo-set', null, main.manual_lat, main.manual_lon).then(function(r) {
					if (r.error) notify(_('Mode switch failed'), r.error);
				});
			}
		};

		so.option(form.Value, 'manual_lat', _('Manual latitude'));
		so.option(form.Value, 'manual_lon', _('Manual longitude'));

		/* ---------- 4+5. Current location + GPS (read-only) ---------- */
		var geoCard = map.section(form.GridSection, 'main', 'wloc-service');
		geoCard.anonymous = true;
		geoCard.addremove = false;
		geoCard.title = _('Current location');

		var geo = status.geo || {};
		geoCard.option(form.DummyValue, '_country', _('Country')).value = geo.country_code || '-';
		geoCard.option(form.DummyValue, '_city', _('City')).value = geo.city || '-';
		geoCard.option(form.DummyValue, '_tz', _('Timezone')).value = geo.timezone || '-';
		geoCard.option(form.DummyValue, '_gps', _('GPS (lat / lon)')).value = gpsOf(geo);
		geoCard.option(form.DummyValue, '_geoState', _('Geo state')).value = geo.state || '-';
		geoCard.option(form.DummyValue, '_observed', _('Observed at')).value = fmtTime(status.observed_at);
		geoCard.option(form.DummyValue, '_exit', _('Exit IP')).value =
			(status.exit && status.exit.ip) ? status.exit.ip : '-';

		/* ---------- 6. Manual search + coordinate apply ---------- */
		var searchCard = map.section(form.NamedSection, 'main', 'wloc-service');
		searchCard.anonymous = true;
		searchCard.title = _('Manual search');

		var queryField = E('input', {
			'class': 'cbi-input-text',
			'id': 'wloc-search-query',
			'type': 'text',
			'placeholder': _('e.g. London, UK')
		});
		var searchBtn = E('button', {
			'class': 'cbi-button cbi-button-apply',
			'id': 'wloc-search-btn',
			'click': function() {
				var q = document.getElementById('wloc-search-query').value.trim();
				if (!q) return;
				this.disabled = true;
				callCtl('geo-set', q, null, null).then(function(r) {
					searchBtn.disabled = false;
					if (r.error) {
						notify(_('Search failed'), r.error);
						return;
					}
					uci.set('wloc-service', 'main', 'geo_source', 'manual');
					uci.save('wloc-service');
					uci.commit('wloc-service');
					notify(_('Applied'), _('Search result is now the active location. Verify on the iPhone.'));
				});
			}
		}, _('Search and apply'));

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
						notify(_('Apply failed'), r.error);
						return;
					}
					uci.set('wloc-service', 'main', 'manual_lat', lat);
					uci.set('wloc-service', 'main', 'manual_lon', lon);
					uci.set('wloc-service', 'main', 'geo_source', 'manual');
					uci.save('wloc-service');
					uci.commit('wloc-service');
					notify(_('Applied'), _('Coordinates are now the active location.'));
				});
			}
		}, _('Apply coordinates'));

		var searchBox = E('div', { 'class': 'cbi-section' },
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' },
					E('label', { 'for': 'wloc-search-query', 'class': 'cbi-value-title' },
						_('Place name'), ' ', _('(online search)')),
					E('div', { 'class': 'cbi-value-field' }, queryField, ' ', searchBtn))),
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' },
					E('label', { 'class': 'cbi-value-title' }, _('Or enter coordinates')),
					E('div', { 'class': 'cbi-value-field' }, coordLat, ' ', coordLon, ' ', coordBtn))));

		/* ---------- 7. Presets ---------- */
		var presetsCard = map.section(form.GridSection, 'preset');
		presetsCard.title = _('Saved locations');
		presetsCard.addremove = true;
		presetsCard.anonymous = true;

		presetsCard.option(form.Value, 'label', _('Label'));
		presetsCard.option(form.Value, 'latitude', _('Latitude'));
		presetsCard.option(form.Value, 'longitude', _('Longitude'));

		presetsCard.option(form.Button, '_apply', _('Apply')).onclick = function(section_id) {
			var s = uci.get('wloc-service', section_id);
			if (!s || !s.latitude || !s.longitude) {
				notify(_('Apply failed'), _('Preset has no coordinates.'));
				return;
			}
			uci.set('wloc-service', 'main', 'manual_lat', s.latitude);
			uci.set('wloc-service', 'main', 'manual_lon', s.longitude);
			uci.set('wloc-service', 'main', 'geo_source', 'manual');
			uci.save('wloc-service');
			uci.commit('wloc-service');
			callCtl('geo-set', null, s.latitude, s.longitude).then(function(r) {
				if (r.error) notify(_('Apply failed'), r.error);
				else notify(_('Applied'), _('Preset is now the active location.'));
			});
		};

		/* ---------- 1. Safari certificate ---------- */
		var certCard = map.section(form.NamedSection, 'main', 'wloc-service');
		certCard.anonymous = true;
		certCard.title = _('Certificate (Safari install)');

		var certLink = E('a', { 'href': PROFILE_URL, 'target': '_blank', 'id': 'wloc-cert-link' }, PROFILE_URL);
		var certText = E('div', { 'class': 'cbi-value' },
			E('label', { 'class': 'cbi-value-title' }, _('Profile link')),
			E('div', { 'class': 'cbi-value-field' }, certLink));

		var regenBtn = E('button', {
			'class': 'cbi-button cbi-button-apply',
			'id': 'wloc-regen-btn',
			'click': function() {
				this.disabled = true;
				regenProfile().then(function(r) {
					regenBtn.disabled = false;
					if (r.error) {
						notify(_('Regenerate failed'), r.error);
						return;
					}
					notify(_('Profile ready'),
						_('On the iPhone open Safari and visit %s, then enable full trust in ' +
						  'Settings > General > About > Certificate Trust Settings.')
							.format(r.url || PROFILE_URL));
				});
			}
		}, _('Regenerate profile'));

		var certBox = E('div', { 'class': 'cbi-section' },
			certText,
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' },
					E('label', { 'class': 'cbi-value-title' }, _('Install steps')),
					E('div', { 'class': 'cbi-value-field' },
						_('1. Open the profile link in Safari on the test iPhone. ') +
						_('2. Install the configuration profile. ') +
						_('3. Enable full trust in Settings > General > About > Certificate Trust Settings.')))),
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' },
					E('label', { 'class': 'cbi-value-title' }, ''),
					E('div', { 'class': 'cbi-value-field' }, regenBtn))));

		/* ---------- 8. Usage log ---------- */
		var logRows = [];
		eventsText.split('\n').forEach(function(line) {
			line = line.trim();
			if (!line) return;
			try { logRows.push(JSON.parse(line)); } catch (e) {}
		});
		logRows = logRows.slice(-20).reverse();

		var tbody = E('tbody', { 'id': 'wloc-events-body' });
		var table = E('table', { 'class': 'cbi-section-table' },
			E('thead', {},
				E('tr', {},
					E('th', {}, _('Time')),
					E('th', {}, _('Event')),
					E('th', {}, _('Location')),
					E('th', {}, _('Source')))),
			tbody);

		function renderEvents() {
			tbody.innerHTML = '';
			if (!logRows.length) {
				tbody.appendChild(E('tr', {}, E('td', { 'colspan': 4 }, _('No events yet.'))));
				return;
			}
			logRows.forEach(function(ev) {
				var where = '-';
				if (ev.city || ev.country_code)
					where = (ev.city || '') + (ev.country_code ? ' (' + ev.country_code + ')' : '');
				else if (ev.latitude != null && ev.longitude != null)
					where = ev.latitude.toFixed(4) + ', ' + ev.longitude.toFixed(4);
				tbody.appendChild(E('tr', {},
					E('td', {}, fmtTime(ev.time)),
					E('td', {}, ev.type || '-'),
					E('td', {}, where),
					E('td', {}, ev.source || '-')));
			});
		}
		renderEvents();

		var logBox = E('div', { 'class': 'cbi-section' },
			E('h3', {}, _('WLOC usage log')),
			table);

		map.contents.appendChild(searchBox);
		map.contents.appendChild(certBox);
		map.contents.appendChild(logBox);

		/* ---------- Live refresh (status + log) ---------- */
		var ids = {
			'_phase': 'cbid.wloc-service.main._phase',
			'_country': 'cbid.wloc-service.main._country',
			'_city': 'cbid.wloc-service.main._city',
			'_tz': 'cbid.wloc-service.main._tz',
			'_gps': 'cbid.wloc-service.main._gps',
			'_geoState': 'cbid.wloc-service.main._geoState',
			'_observed': 'cbid.wloc-service.main._observed',
			'_exit': 'cbid.wloc-service.main._exit'
		};

		poll.add(function() {
			return fs.read(STATUS_FILE).then(function(text) {
				var s;
				try { s = JSON.parse(text); } catch (e) { return; }
				var g = s.geo || {};
				var values = {
					'_phase': s.service_phase || '-',
					'_country': g.country_code || '-',
					'_city': g.city || '-',
					'_tz': g.timezone || '-',
					'_gps': gpsOf(g),
					'_geoState': g.state || '-',
					'_observed': fmtTime(s.observed_at),
					'_exit': (s.exit && s.exit.ip) ? s.exit.ip : '-'
				};
				for (var key in values) {
					var el = document.getElementById(ids[key]);
					if (el) el.value = values[key];
				}
			}).catch(function() {});
		}, 5);

		poll.add(function() {
			return fs.read(EVENTS_FILE).then(function(text) {
				var rows = [];
				text.split('\n').forEach(function(line) {
					line = line.trim();
					if (!line) return;
					try { rows.push(JSON.parse(line)); } catch (e) {}
				});
				logRows = rows.slice(-20).reverse();
				renderEvents();
			}).catch(function() {});
		}, 5);

		return map;
	}
});
