'use strict';
'require view';
'require fs';
'require poll';
'require dom';
'require ui';
'require uci';
'require wificalling-location-gateway.i18n as i18n';

function parseStatus(value) {
	try { return JSON.parse(value); } catch (e) { return { devices: [] }; }
}

function when(epoch) {
	return epoch ? new Date(epoch * 1000).toLocaleString() : '-';
}

function wfcLabel(value) {
	switch (value) {
	case 'registered': return _('Registered');
	case 'connecting': return _('Connecting');
	case 'not_detected': return _('Not detected');
	case 'likely_registered': return _('Likely registered');
	case 'active_traffic': return _('Active traffic');
	case 'nat_t_seen': return _('NAT-T seen');
	case 'negotiating': return _('Negotiating');
	case 'no_session': return _('No session');
	default: return value || '-';
	}
}

function statusRows(source) {
	return (source.devices || []).map(function(device) {
		var values = [
			device.label,
			device.ip,
			wfcLabel(device.wificalling || device.state),
			device.node || '-',
			device.epdg_ip || '-',
			(device.ike_seen ? '500' : '-') + ' / ' + (device.nat_t_seen ? '4500' : '-'),
			device.assured ? _('Yes') : _('No'),
			device.sent_packets + ' ↑ / ' + device.reply_packets + ' ↓',
			when(device.last_activity)
		];
		return E('tr', { class: 'tr' }, values.map(function(value) {
			return E('td', { class: 'td' }, String(value));
		}));
	});
}

function eventLines(value) {
	return value.trim() ? value.trim().split('\n').reverse() : [];
}

function eventFields(line) {
	try {
		var event = JSON.parse(line), fields = event.fields || {};
		if (event.event_code)
			return [event.timestamp || 0, event.profile_scope || '-', '-', event.event_code,
				fields.delta_sent || 0, fields.delta_reply || 0, 'call_or_sms_unknown', fields.state || '-'];
	} catch (e) {}
	return line.split('|');
}

function activityLabel(value) {
	switch (value) {
	case 'handshake_success': return _('Handshake success');
	case 'handshake_failed': return _('Handshake failed');
	case 'sustained_traffic': return _('Sustained traffic');
	default: return value || '-';
	}
}

function meaningLabel(value) {
	return value === 'likely_call'
		? _('Call in progress (inferred from sustained encrypted traffic)')
		: _('Encrypted activity; call/SMS unknown');
}

function eventRows(value) {
	return eventLines(value).map(function(line) {
		var fields = eventFields(line);
		return E('tr', { class: 'tr' }, [
			when(Number(fields[0])), fields[1], fields[2], wfcLabel(fields[7]),
			activityLabel(fields[3]), (fields[4] || '0') + ' ↑ / ' + (fields[5] || '0') + ' ↓',
			meaningLabel(fields[6])
		].map(function(value) { return E('td', { class: 'td' }, String(value)); }));
	});
}

return view.extend({
	load: function() {
		return Promise.all([
			L.resolveDefault(fs.read('/var/run/wificalling-gateway/status.json'), '{}'),
			L.resolveDefault(fs.read('/var/run/wificalling-gateway/events.log'), ''),
			uci.load('wificalling-gateway')
		]);
	},

	render: function(data) {
		i18n.localizeTabs();
		var statusBody = E('tbody', {}, statusRows(parseStatus(data[0])));
		var eventBody = E('tbody', {}, eventRows(data[1] || ''));
		var eventCount = E('span', {}, String(eventLines(data[1] || '').length));
		var logEnabled = uci.get('wificalling-gateway', 'main', 'log_enabled');

		function refresh(statusText, eventsText) {
			dom.content(statusBody, statusRows(parseStatus(statusText)));
			dom.content(eventBody, eventRows(eventsText));
			dom.content(eventCount, String(eventLines(eventsText).length));
		}

		var clear = E('button', { class: 'btn cbi-button-negative', click: function() {
			ui.showModal(_('Clear activity log?'), [
				E('p', {}, _('This permanently removes only the Wi-Fi Calling activity history. Settings and system logs are not affected.')),
				E('div', { class: 'right' }, [
					E('button', { class: 'btn', click: ui.hideModal }, _('Cancel')),
					E('button', { class: 'btn cbi-button-negative', click: function() {
						fs.write('/var/run/wificalling-gateway/events.log', '').then(function() {
							refresh(data[0], '');
							ui.hideModal();
							ui.addNotification(null, E('p', {}, _('Activity log cleared.')), 'info');
						}).catch(function(error) {
							ui.addNotification(null, E('p', {}, _('Unable to clear log:') + ' ' + error.message), 'error');
						});
					} }, _('Clear log'))
				])
			]);
		} }, _('Clear log'));

		poll.add(function() {
			return Promise.all([
				L.resolveDefault(fs.read('/var/run/wificalling-gateway/status.json'), '{}'),
				L.resolveDefault(fs.read('/var/run/wificalling-gateway/events.log'), '')
			]).then(function(values) { refresh(values[0], values[1]); });
		}, 5);

		return E([], [
			E('h2', {}, _('WCG Status & Logs')),
			E('p', {}, _('WiFi Calling Gateway status and activity logs are combined here for one operational view.')),
			E('div', { class: 'cbi-section' }, [
				E('h3', {}, _('Wi-Fi Calling status')),
				E('p', {}, _('Registered means an ASSURED bidirectional UDP 4500 tunnel was observed. This is network evidence, not carrier activation confirmation.')),
				E('table', { class: 'table' }, [
					E('tr', { class: 'tr table-titles' }, [_('Device'), _('IP'), _('Wi-Fi Calling status'), _('Node'), _('ePDG IP'), _('UDP 500/4500'), _('ASSURED'), _('Packets'), _('Last activity')].map(function(value) { return E('th', { class: 'th' }, value); })),
					statusBody
				])
			]),
			E('div', { class: 'cbi-section' }, [
				E('h3', {}, _('Encrypted IMS activity log')),
				E('p', {}, _('Records handshake success or failure and sustained encrypted communication such as ringing or calls. Brief traffic bursts are not logged. The tunnel content is encrypted: a call is inferred from sustained bidirectional traffic, SMS cannot be distinguished, and phone numbers or message content are never visible.')),
				logEnabled === '0' ? E('div', { class: 'alert-message warning' }, _('Activity log recording is disabled. Enable it in WCG Setting.')) : '',
				E('p', {}, [_('Records:'), ' ', eventCount, ' ', clear]),
				E('table', { class: 'table' }, [
					E('tr', { class: 'tr table-titles' }, [_('Time'), _('Device'), _('IP'), _('Wi-Fi Calling'), _('Activity'), _('Packet delta'), _('Meaning')].map(function(value) { return E('th', { class: 'th' }, value); })),
					eventBody
				])
			])
		]);
	}
});
