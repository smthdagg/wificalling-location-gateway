'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require form';
'require fs';
'require poll';
'require uci';
'require dom';
'require ui';
'require wificalling-gateway.node-import as nodeImport';

return view.extend({
	load: function() {
		return Promise.all([
			L.resolveDefault(fs.read('/var/run/wificalling-gateway/node-status.json'), '{}'),
			uci.load('wificalling-gateway'),
			L.resolveDefault(fs.read('/tmp/dhcp.leases'), ''),
			uci.load('dhcp')
		]);
	},
	render: function(data) {
		wlocI18n.localizeTabs();
		var nodeParsed;
		try { nodeParsed = JSON.parse(data[0]); } catch (e) { nodeParsed = { nodes: [] }; }
		function nodeById(id, source) {
			var nodes = (source || nodeParsed).nodes || [];
			for (var i = 0; i < nodes.length; i++) if (nodes[i].id === id) return nodes[i];
			return null;
		}
		function quality(n) {
			if (!n) return '-';
			if (n.state === 'unreachable') return wlocI18n.t('Offline');
			if (n.ping_ms == null) return wlocI18n.t('Unknown');
			if (n.ping_ms <= 100) return wlocI18n.t('Excellent');
			if (n.ping_ms <= 200) return wlocI18n.t('Good');
			if (n.ping_ms <= 300) return wlocI18n.t('Fair');
			return wlocI18n.t('Poor');
		}
		function nodeState(n) {
			if (!n) return '-';
			if (n.state === 'reachable' || n.state === 'tcp_reachable') return wlocI18n.t('Alive');
			if (n.state === 'unreachable') return wlocI18n.t('Offline');
			return wlocI18n.t('Unknown');
		}
		function latency(n) { return n && n.ping_ms != null ? n.ping_ms + ' ms (' + n.measurement + ')' : '-'; }
		// Live DHCP lease map (IP -> MAC) and plugin-managed static bindings
		// (wfc_ host sections) for the device policy status column.  dnsmasq
		// lease lines are: expiry MAC IP hostname clientid.
		var leaseMac = {};
		(data[2] || '').split('\n').forEach(function(line) {
			var p = line.split(/\s+/);
			if (p.length >= 3 && /^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$/.test(p[2]))
				leaseMac[p[2]] = p[1];
		});
		var wfcHost = {};
		uci.sections('dhcp', 'host').forEach(function(h) {
			if ((h['.name'] || '').indexOf('wfc_') === 0 && h.ip)
				wfcHost[h.ip] = { mac: h.mac || '', name: h.name || '' };
		});
		function dhcpState(ip) {
			var mac = leaseMac[ip], host = wfcHost[ip];
			if (host && host.mac && mac && host.mac.toLowerCase() === mac.toLowerCase()) return wlocI18n.t('Bound');
			if (host && host.mac && mac) return wlocI18n.t('MAC changed, rebind on reconnect');
			if (mac) return wlocI18n.t('Not bound yet');
			return wlocI18n.t('Device offline');
		}

		var m = new form.Map('wificalling-gateway', wlocI18n.t('Wi-Fi Calling Gateway settings'),
			wlocI18n.t('Configure proxy nodes and assign fixed LAN devices. Monitoring and logs are available from the submenu.'));
		var importPanel = E('div', { class: 'cbi-section' }, [
			E('h3', {}, wlocI18n.t('Import proxy node')),
			E('p', {}, wlocI18n.t('Paste one AnyTLS, Hysteria2/Hy2, TUIC, VLESS, VMess, Trojan, or WireGuard (wg://) link. It is parsed locally in this browser and is not sent to an external service.')),
			E('div', { class: 'cbi-section-create' }, [
				E('button', { class: 'cbi-button cbi-button-add', click: function() {
				var input = E('textarea', { class: 'cbi-input-textarea', rows: 6, style: 'width:100%', placeholder: 'anytls://…' });
				ui.showModal(wlocI18n.t('Import node link'), [input, E('div', { class: 'right' }, [
					E('button', { class: 'btn', click: ui.hideModal }, wlocI18n.t('Cancel')),
					E('button', { class: 'btn cbi-button-positive', click: function() {
						var parsed;
						try { parsed = nodeImport.parse(input.value); }
						catch (err) { ui.addNotification(null, E('p', {}, wlocI18n.t('Unable to parse node link:') + ' ' + err.message), 'error'); return; }
						var sid = uci.add('wificalling-gateway', 'node');
						Object.keys(parsed).forEach(function(key) { if (parsed[key] !== '') uci.set('wificalling-gateway', sid, key, parsed[key]); });
						uci.save().then(function() {
							ui.hideModal();
							ui.addNotification(null, E('p', {}, wlocI18n.t('Node imported successfully. Reloading settings…')), 'info');
							window.setTimeout(function() { window.location.reload(); }, 500);
						}).catch(function(err) { ui.addNotification(null, E('p', {}, wlocI18n.t('Unable to save imported node:') + ' ' + err.message), 'error'); });
					} }, wlocI18n.t('Import'))
				])]);
				} }, wlocI18n.t('Import node link'))
			])
		]);
		var s = m.section(form.NamedSection, 'main', 'global', wlocI18n.t('General'));
		s.option(form.Flag, 'enabled', wlocI18n.t('Enable'));
		var logLevel = s.option(form.ListValue, 'log_level', wlocI18n.t('Log level'));
		logLevel.value('warn', wlocI18n.t('Warning')); logLevel.value('info', wlocI18n.t('Information')); logLevel.value('debug', wlocI18n.t('Debug'));
		var logEnabled = s.option(form.Flag, 'log_enabled', wlocI18n.t('Activity log'));
		logEnabled.default = '1';
		logEnabled.description = wlocI18n.t('Record handshake outcomes and sustained encrypted communication. Turn off to stop writing the activity log.');
		var eventInterval = s.option(form.Value, 'event_interval', wlocI18n.t('Sustained activity log interval (seconds)'));
		eventInterval.datatype = 'range(30,3600)'; eventInterval.default = '60';
		eventInterval.depends('log_enabled', '1');
		eventInterval.description = wlocI18n.t('Continuous traffic is aggregated and written at most once per interval.');
		var maxEvents = s.option(form.Value, 'max_events_per_device', wlocI18n.t('Maximum records per device'));
		maxEvents.datatype = 'range(1,500)'; maxEvents.default = '20';
		maxEvents.depends('log_enabled', '1');
		maxEvents.description = wlocI18n.t('Each device keeps its own newest records, so one device cannot fill the entire log.');

		s = m.section(form.GridSection, 'node', wlocI18n.t('Proxy nodes'));
		s.addremove = true; s.nodescriptions = true; s.anonymous = true; s.addbtntitle = wlocI18n.t('Add proxy node');
		s.sectiontitle = function(id) { return uci.get('wificalling-gateway', id, 'label') || id; };
		s.option(form.Flag, 'enabled', wlocI18n.t('Enable')).default = '1';
		var nodeLabel = s.option(form.Value, 'label', wlocI18n.t('Node display name'));
		nodeLabel.rmempty = false; nodeLabel.placeholder = wlocI18n.t('Example: UK AnyTLS');
		nodeLabel.description = wlocI18n.t('This name is shown in the device node selector.');
		var p = s.option(form.ListValue, 'protocol', wlocI18n.t('Protocol'));
		['anytls','hysteria2','tuic','vless','vmess','trojan','wireguard'].forEach(function(x) { p.value(x); });
		s.option(form.Value, 'server', wlocI18n.t('Server')).datatype = 'host';
		s.option(form.Value, 'port', wlocI18n.t('Port')).datatype = 'port';
		var nodeStatus = s.option(form.DummyValue, '_node_status', wlocI18n.t('Node status'));
		nodeStatus.textvalue = function(id) { return E('span', { id: 'wfc-node-state-' + id }, nodeState(nodeById(id))); };
		var nodePing = s.option(form.DummyValue, '_node_ping', wlocI18n.t('Ping / latency'));
		nodePing.textvalue = function(id) { return E('span', { id: 'wfc-node-ping-' + id }, latency(nodeById(id))); };
		var nodeQuality = s.option(form.DummyValue, '_node_quality', wlocI18n.t('Quality'));
		nodeQuality.textvalue = function(id) { return E('span', { id: 'wfc-node-quality-' + id }, quality(nodeById(id))); };
		var secret = s.option(form.Value, 'password', wlocI18n.t('Password'));
		secret.password = true; secret.textvalue = function(id) { return this.cfgvalue(id) ? wlocI18n.t('Set') : wlocI18n.t('Not set'); };
		var uuidField = s.option(form.Value, 'uuid', wlocI18n.t('UUID'));
		uuidField.password = true; uuidField.textvalue = function(id) { return this.cfgvalue(id) ? wlocI18n.t('Set') : wlocI18n.t('Not set'); };
		s.option(form.Value, 'sni', wlocI18n.t('TLS server name'));
		var securityOpt = s.option(form.ListValue, 'security', wlocI18n.t('Security'));
		securityOpt.value('', wlocI18n.t('None')); securityOpt.value('tls'); securityOpt.value('reality');
		securityOpt.depends('protocol', 'vless');
		securityOpt.depends('protocol', 'vmess');
		// The compiler has no reality arm for VMess; selecting it would emit a
		// cleartext outbound that sing-box check accepts. Reject it up front.
		securityOpt.validate = function(section_id, value) {
			if (value == 'reality' && this.map.getSectionValue(section_id, 'protocol') == 'vmess')
				return false;
			return true;
		};
		s.option(form.Flag, 'insecure', wlocI18n.t('Allow insecure certificate'));
		s.option(form.Value, 'alpn', wlocI18n.t('ALPN'));
		s.option(form.Value, 'pin_sha256', wlocI18n.t('TLS public-key SHA-256 (base64)'));
		s.option(form.Value, 'flow', wlocI18n.t('VLESS flow'));
		s.option(form.Value, 'public_key', wlocI18n.t('Reality public key'));
		s.option(form.Value, 'short_id', wlocI18n.t('Reality short ID'));
		s.option(form.Value, 'fingerprint', wlocI18n.t('Reality fingerprint'));
		var udpMode = s.option(form.ListValue, 'udp_mode', wlocI18n.t('TUIC UDP mode'));
		udpMode.value('native', wlocI18n.t('Native')); udpMode.value('quic', wlocI18n.t('QUIC'));
		var transport = s.option(form.ListValue, 'transport', wlocI18n.t('Transport'));
		transport.value('', wlocI18n.t('None')); transport.value('ws', wlocI18n.t('WebSocket'));
		s.option(form.Value, 'path', wlocI18n.t('WebSocket path'));
		s.option(form.Value, 'host', wlocI18n.t('WebSocket Host'));
		var wgKey = s.option(form.Value, 'private_key', wlocI18n.t('WireGuard private key'));
		wgKey.password = true; wgKey.textvalue = function(id) { return this.cfgvalue(id) ? wlocI18n.t('Set') : wlocI18n.t('Not set'); };
		s.option(form.Value, 'local_address', wlocI18n.t('WireGuard local address'));
		s.option(form.Value, 'reserved', wlocI18n.t('WireGuard reserved (comma-separated)'));
		s.option(form.Value, 'mtu', wlocI18n.t('WireGuard MTU'));

		s = m.section(form.GridSection, 'device', wlocI18n.t('Device policies'));
		s.addremove = true; s.nodescriptions = true; s.anonymous = true; s.addbtntitle = wlocI18n.t('Add LAN device');
		s.sectiontitle = function(id) { return uci.get('wificalling-gateway', id, 'label') || id; };
		s.option(form.Flag, 'enabled', wlocI18n.t('Enable')).default = '1';
		var deviceLabel = s.option(form.Value, 'label', wlocI18n.t('Device display name'));
		deviceLabel.rmempty = false; deviceLabel.placeholder = wlocI18n.t('Example: iPhone 12');
		var routeMode = s.option(form.ListValue, 'route_mode', wlocI18n.t('Routing mode'));
		routeMode.value('independent', wlocI18n.t('Independent tunnel')); routeMode.value('follow_gateway', wlocI18n.t('Follow gateway'));
		routeMode.default = 'independent';
		var selectedNode = s.option(form.ListValue, 'node', wlocI18n.t('Node'));
		selectedNode.rmempty = false; selectedNode.depends('route_mode', 'independent');
		selectedNode.description = wlocI18n.t('Save the node first, then reload this page to select it for a device.');
		uci.sections('wificalling-gateway', 'node').forEach(function(node) { selectedNode.value(node['.name'], node.label || node['.name']); });
		var ips = s.option(form.DynamicList, 'source_ip', wlocI18n.t('LAN IPv4 addresses'));
		ips.datatype = 'ip4addr'; ips.rmempty = false; ips.placeholder = '192.168.31.x';
				var dhcpBinding = s.option(form.DummyValue, '_dhcp_binding', wlocI18n.t('DHCP binding'));
				// A DummyValue has no editable value: without rmempty the save
				// parse rejects it as "must not be empty", silently breaking the
				// "Save" button (Save & Apply still worked via the staged-changes
				// fallback).  The grid row renders via textvalue; the edit modal
				// renders the widget with cfgvalue (always null), so renderWidget
				// is overridden to show the same live state in both places.
				dhcpBinding.rmempty = true;
				function bindingState(id) {
					if ((uci.get('wificalling-gateway', id, 'route_mode') || 'independent') !== 'independent')
						return wlocI18n.t('Following gateway');
					var ipList = uci.get('wificalling-gateway', id, 'source_ip') || [];
					if (!Array.isArray(ipList)) ipList = [ipList];
					return ipList.map(function(ip) { return ip + ': ' + dhcpState(ip); }).join('<br>');
				}
				// Grid row renders via textvalue; the edit modal renders the widget
				// with cfgvalue (always null for a DummyValue), so override
				// renderWidget to show the same live state in both places.
				dhcpBinding.rawhtml = true;
				dhcpBinding.textvalue = function(id) { return bindingState(id); };
				dhcpBinding.renderWidget = function(section_id, option_index, cfgvalue) {
					return E('output', { 'for': this.cbid(section_id) }, bindingState(section_id));
				};

		poll.add(function() {
			return L.resolveDefault(fs.read('/var/run/wificalling-gateway/node-status.json'), '{}').then(function(raw) {
				var current; try { current = JSON.parse(raw); } catch (e) { current = { nodes: [] }; }
				(current.nodes || []).forEach(function(n) {
					[['state', nodeState(n)], ['ping', latency(n)], ['quality', quality(n)]].forEach(function(v) {
						var el = document.getElementById('wfc-node-' + v[0] + '-' + n.id); if (el) dom.content(el, v[1]);
					});
				});
			});
		}, 5);
		this.mapInstance = m;
		return m.render().then(function(formNode) {
			// Place the import panel between the proxy-node table (whose
			// "Add proxy node" button lives in the table header) and the
			// device-policy table: insert it right before the device
			// section. Falls back to after the form if the section cannot
			// be located.
			var deviceSection = formNode.querySelector('#cbi-wificalling-gateway-device');
			var importMoved = false;
			if (deviceSection && deviceSection.parentNode) {
				deviceSection.parentNode.insertBefore(importPanel, deviceSection);
				importMoved = true;
			}
			var nodes = importMoved ? E([], [formNode]) : E([], [formNode, importPanel]);
			// LuCI 24.10's footer "Save" button handler is resolved through
			// the view prototype during footer creation; on this firmware it
			// ends up unbound (the button does nothing, while "Save & Apply"
			// still works via the staged-changes fallback).  Bind the form
			// save directly once the footer exists.
			window.setTimeout(function() {
				var btn = document.querySelector('#view button.cbi-button-save');
				if (btn && !btn._wfcSaveBound) {
					btn._wfcSaveBound = true;
					// The LuCI 24.10 default "Save" handler resolves the Map
					// through a DOM instance lookup that fails on this
					// firmware, and Map.save() alone never commits the
					// session-scoped UCI changeset anyway (only apply does).
					// Bind save + apply directly so plain "Save" persists
					// the configuration like "Save & Apply".
					btn.addEventListener('click', function(ev) {
						ev.preventDefault();
						ev.stopPropagation();
						m.save().then(function() {
							return ui.changes.apply(true);
						}).catch(function() {});
					});
				}
			}, 200);
			return nodes;
		});
	},
	handleSave: function(ev) {
		// The LuCI 24.10 default resolves the Map through a DOM instance
		// lookup that silently fails on this firmware, so the "Save"
		// button did nothing while "Save & Apply" still worked (apply
		// commits the staged changes as a fallback).  Save through the
		// form instance directly instead.
		return this.mapInstance ? this.mapInstance.save() : Promise.resolve();
	}
});
