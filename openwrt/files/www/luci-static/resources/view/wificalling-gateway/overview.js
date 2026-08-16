'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require form';
'require fs';
'require poll';
'require rpc';
'require uci';
'require dom';
'require ui';
'require wificalling-gateway.node-import as nodeImport';

var nodeTestRpc = rpc.declare({
	object: 'luci.wloc',
	method: 'node_test',
	params: ['id'],
	expect: {}
});

return view.extend({
	load: function() {
		// The node status file is exported under the uhttpd docroot and
		// read with a plain GET: the /ubus JSON-RPC channel truncates
		// larger replies on some firmwares, leaving the status blank.
		return Promise.all([
			L.resolveDefault(fetch('/wloc-node-status.json').then(function(r) { return r.text(); }), '{}'),
			uci.load('wificalling-gateway'),
			L.resolveDefault(fs.read('/tmp/dhcp.leases'), ''),
			uci.load('dhcp'),
			L.resolveDefault(fs.read('/proc/net/arp'), '')
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
			if (n.state === 'handshake_ok') return wlocI18n.t('Good');
			if (n.state === 'handshake_failed') return wlocI18n.t('Offline');
			if (n.state === 'unreachable') return wlocI18n.t('Offline');
			// ping_ms may arrive as a JSON number or a quoted string.
			var ms = parseFloat(n.ping_ms);
			if (isNaN(ms)) return wlocI18n.t('Unknown');
			if (ms <= 100) return wlocI18n.t('Excellent');
			if (ms <= 200) return wlocI18n.t('Good');
			if (ms <= 300) return wlocI18n.t('Fair');
			return wlocI18n.t('Poor');
		}
		function nodeState(n) {
			if (!n) return '-';
			if (n.state === 'handshake_ok') return wlocI18n.t('Handshake OK');
			if (n.state === 'handshake_failed') {
				// The status export classifies failed handshakes so a bad
				// node is visible as such instead of a bare "Offline".
				// Keep the cell short; the full explanation is a tooltip.
				var reason = wgFailReason(n.reason);
				return wlocI18n.t('Handshake failed') + (reason ? ' (' + reason + ')' : '');
			}
			if (n.state === 'reachable' || n.state === 'tcp_reachable') return wlocI18n.t('Alive');
			if (n.state === 'unreachable') return wlocI18n.t('Offline');
			return wlocI18n.t('Unknown');
		}
		// Short status-cell label for a failed handshake.
		function wgFailReason(reason) {
			if (reason === 'config_missing') return wlocI18n.t('Missing config');
			if (reason === 'timeout') return wlocI18n.t('Timeout');
			if (reason === 'unreachable') return wlocI18n.t('Unreachable');
			return reason || '';
		}
		// Full explanation for the tooltip / result banner.
		function wgFailDetail(reason) {
			if (reason === 'config_missing') return wlocI18n.t('Missing key/address');
			if (reason === 'timeout') return wlocI18n.t('Handshake timed out (key/psk mismatch?)');
			if (reason === 'unreachable') return wlocI18n.t('Server unreachable');
			return '';
		}
		// Manual connection test: asks the router to run a fresh check
		// right now - a WireGuard handshake (bypassing the monitor's
		// result cache) or a TCP reachability probe for other protocols -
		// and reports the exit IP or the failure reason.
		// The result banner carries an explicit close button (the stock
		// LuCI notification's dismiss control is easy to miss under some
		// themes) and never auto-dismisses.
		function testNotify(message, kind, detail) {
			var mc = document.querySelector('#maincontent') || document.body;
			var msg = E('div', {
				'class': 'alert-message fade-in ' + (kind || 'info'),
				style: 'display:flex;align-items:center;padding:8px 12px',
				title: detail || null
			}, [
				E('div', { style: 'flex:1' }, E('p', { style: 'margin:0' }, message)),
				E('button', {
					'class': 'btn',
					style: 'margin-left:12px;white-space:nowrap',
					click: function() {
						msg.classList.add('fade-out');
						window.setTimeout(function() {
							if (msg.parentNode) msg.parentNode.removeChild(msg);
						}, 400);
					}
				}, wlocI18n.t('Close'))
			]);
			mc.insertBefore(msg, mc.firstElementChild);
			return msg;
		}
		function runNodeTest(id, btn) {
			if (btn.disabled) return;
			btn.disabled = true;
			var original = btn.textContent;
			btn.textContent = wlocI18n.t('Testing…');
			nodeTestRpc(id).then(function(r) {
				btn.disabled = false;
				btn.textContent = original;
				if (r && r.state === 'handshake_ok') {
					testNotify(wlocI18n.t('Handshake OK') + ' — ' + r.exit_ip, 'info');
				}
				else if (r && r.state === 'handshake_failed') {
					testNotify(wlocI18n.t('Handshake failed') + ' (' + wgFailReason(r.reason) + ')', 'error', wgFailDetail(r.reason));
				}
				else if (r && r.state === 'tcp_reachable') {
					testNotify(wlocI18n.t('Alive') + (r.ping_ms ? ' — ' + r.ping_ms + ' ms' : ''), 'info');
				}
				else if (r && r.state === 'unreachable') {
					testNotify(wlocI18n.t('Offline'), 'error');
				}
				else {
					testNotify(wlocI18n.t('Unable to test node: ') + wgFailReason(r && r.reason), 'error');
				}
			}).catch(function(e) {
				btn.disabled = false;
				btn.textContent = original;
				testNotify(wlocI18n.t('Unable to test node: ') + String(e), 'error');
			});
		}
		function latency(n) {
			if (!n) return '-';
			// WireGuard handshake rows carry the verified exit IP instead
			// of an ICMP latency.
			if (n.measurement === 'wg_handshake') return n.ping_ms || '-';
			return n.ping_ms != null ? n.ping_ms + ' ms (' + n.measurement + ')' : '-';
		}
		// Live DHCP lease map (IP -> MAC) and plugin-managed static bindings
		// (wfc_ host sections) for the device policy status column.  dnsmasq
		// lease lines are: expiry MAC IP hostname clientid.
		var leaseMac = {};
		var leaseHost = {};
		(data[2] || '').split('\n').forEach(function(line) {
			var p = line.split(/\s+/);
			if (p.length >= 4 && /^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$/.test(p[2])) {
				leaseMac[p[2]] = p[1];
				if (p[3] && p[3] !== '*')
					leaseHost[p[2]] = p[3];
			}
		});
		// Devices seen in the ARP cache but not in the DHCP leases (static
		// IPs) still show up in the connected-devices picker.
		var arpDevices = {};
		(data[4] || '').split('\n').slice(1).forEach(function(line) {
			var p = line.trim().split(/\s+/);
			if (p.length >= 4 && /^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$/.test(p[0])
				&& /^[0-9a-fA-F:]+$/.test(p[2]))
				arpDevices[p[0]] = p[2];
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
			// No DHCP lease (static IP, or a router that does not run DHCP
			// at all, e.g. a secondary/AP router): the ARP cache is the only
			// liveness source, so a recently-seen device is online, not
			// offline. Only report offline when neither source knows it.
			if (arpDevices[ip]) return wlocI18n.t('Online (static IP)');
			return wlocI18n.t('Device offline');
		}

		// The router's LAN subnet hint for the IP placeholder, derived from
		// the address the admin uses to reach LuCI (e.g. 192.168.31.x).
		function lanSubnetHint() {
			var host = location.hostname || '';
			if (/^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$/.test(host)) {
				var parts = host.split('.');
				return parts.slice(0, 3).join('.') + '.x';
			}
			return '192.168.x.x';
		}

		// Connected LAN devices (DHCP hostname when known, ARP-only entries
		// otherwise) for the add-device picker; the router itself and IPs
		// already bound to a device policy are excluded.
		var detected = {};
		Object.keys(leaseHost).forEach(function(ip) {
			detected[ip] = { name: leaseHost[ip], mac: leaseMac[ip] };
		});
		Object.keys(arpDevices).forEach(function(ip) {
			if (!detected[ip])
				detected[ip] = { name: '', mac: arpDevices[ip] };
		});
		var routerHost = (location.hostname || '').toLowerCase();
		var boundIps = {};
		uci.sections('wificalling-gateway', 'device').forEach(function(d) {
			(d.source_ip || []).forEach(function(ip) { boundIps[ip] = true; });
		});
		var detectedDevices = Object.keys(detected)
			.filter(function(ip) {
				return ip !== routerHost && !boundIps[ip];
			})
			.map(function(ip) { return { ip: ip, name: detected[ip].name }; })
			.sort(function(a, b) {
				var na = (a.name || a.ip).toLowerCase(), nb = (b.name || b.ip).toLowerCase();
				return na < nb ? -1 : (na > nb ? 1 : 0);
			});

		var m = new form.Map('wificalling-gateway', wlocI18n.t('Wi-Fi Calling Gateway settings'),
			wlocI18n.t('Configure proxy nodes and assign fixed LAN devices. Monitoring and logs are available from the submenu.'));

		// Parse a standard WireGuard config block ([Interface]/[Peer]) into
		// the same node object the link importer produces, so a conf file
		// can be pasted directly instead of being converted to wg:// first.
		function parseWireguardConf(text) {
			var section = null, iface = {}, peer = {};
			text.split('\n').forEach(function(line) {
				var t = line.trim();
				if (t === '[Interface]') { section = 'iface'; return; }
				if (t === '[Peer]') { section = 'peer'; return; }
				if (!section || !t || t.indexOf('#') === 0) return;
				var eq = t.indexOf('=');
				if (eq < 0) return;
				var key = t.slice(0, eq).trim(), val = t.slice(eq + 1).trim();
				if (section === 'iface') iface[key] = val; else peer[key] = val;
			});
			if (!iface.PrivateKey || !iface.Address || !peer.PublicKey || !peer.Endpoint)
				throw new Error(wlocI18n.t('WireGuard conf needs PrivateKey, Address, Peer PublicKey and Endpoint'));
			var endpoint = peer.Endpoint.trim().split(':');
			if (endpoint.length !== 2 || !/^[0-9]+$/.test(endpoint[1]))
				throw new Error(wlocI18n.t('Invalid WireGuard endpoint: ') + peer.Endpoint);
			return {
				enabled: '1', protocol: 'wireguard',
				label: 'WireGuard ' + endpoint[0],
				server: endpoint[0], port: endpoint[1],
				public_key: peer.PublicKey,
				private_key: iface.PrivateKey,
				local_address: iface.Address.split(',')[0].trim(),
				reserved: iface.Reserved || '',
				mtu: iface.MTU || '',
				pre_shared_key: peer.PresharedKey || ''
			};
		}

		var importPanel = E('div', { class: 'cbi-section' }, [
			E('h3', {}, wlocI18n.t('Import proxy node')),
			E('p', {}, wlocI18n.t('Paste one AnyTLS, Hysteria2/Hy2, TUIC, VLESS, VMess, Trojan, or WireGuard link (wg:// or an [Interface]/[Peer] config block). It is parsed locally in this browser and is not sent to an external service.')),
			E('div', { class: 'cbi-section-create' }, [
				E('button', { class: 'cbi-button cbi-button-add', click: function() {
				var input = E('textarea', { class: 'cbi-input-textarea', rows: 6, style: 'width:100%', placeholder: 'anytls://…' });
				ui.showModal(wlocI18n.t('Import node link'), [input, E('div', { class: 'right' }, [
					E('button', { class: 'btn', click: ui.hideModal }, wlocI18n.t('Cancel')),
					E('button', { class: 'btn cbi-button-positive', click: function() {
						var parsed;
						try {
							parsed = /^\s*\[Interface\]/m.test(input.value)
								? parseWireguardConf(input.value)
								: nodeImport.parse(input.value);
						}
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
		var nodeLabel = s.option(form.Value, 'label', wlocI18n.t('Name'));
		nodeLabel.rmempty = false; nodeLabel.placeholder = wlocI18n.t('Example: UK AnyTLS');
		nodeLabel.description = wlocI18n.t('This name is shown in the device node selector.');
		// The GridSection already renders a Name column from the section
		// title; showing the label field again would duplicate it.
		nodeLabel.modalonly = true;
		var p = s.option(form.ListValue, 'protocol', wlocI18n.t('Protocol'));
		['anytls','hysteria2','tuic','vless','vmess','trojan','wireguard'].forEach(function(x) { p.value(x); });
		s.option(form.Value, 'server', wlocI18n.t('Server')).datatype = 'host';
		s.option(form.Value, 'port', wlocI18n.t('Port')).datatype = 'port';
		var nodeStatus = s.option(form.DummyValue, '_node_status', wlocI18n.t('Node status'));
		nodeStatus.textvalue = function(id) {
			var n = nodeById(id);
			var detail = (n && n.state === 'handshake_failed') ? wgFailDetail(n.reason) : '';
			return E('span', {
				id: 'wfc-node-state-' + id,
				title: detail || null
			}, nodeState(n));
		};
		var nodePing = s.option(form.DummyValue, '_node_ping', wlocI18n.t('Ping / latency'));
		nodePing.textvalue = function(id) { return E('span', { id: 'wfc-node-ping-' + id }, latency(nodeById(id))); };
		var nodeQuality = s.option(form.DummyValue, '_node_quality', wlocI18n.t('Quality'));
		nodeQuality.textvalue = function(id) { return E('span', { id: 'wfc-node-quality-' + id }, quality(nodeById(id))); };
		// Every remaining field stays editable in the per-node modal but
		// is hidden from the table so rows stay compact (Edit shows them).
		var secret = s.option(form.Value, 'password', wlocI18n.t('Password'));
		secret.password = true; secret.textvalue = function(id) { return this.cfgvalue(id) ? wlocI18n.t('Set') : wlocI18n.t('Not set'); };
		secret.modalonly = true;
		var uuidField = s.option(form.Value, 'uuid', wlocI18n.t('UUID'));
		uuidField.password = true; uuidField.textvalue = function(id) { return this.cfgvalue(id) ? wlocI18n.t('Set') : wlocI18n.t('Not set'); };
		uuidField.modalonly = true;
		var sniOpt = s.option(form.Value, 'sni', wlocI18n.t('TLS server name'));
		sniOpt.modalonly = true;
		var securityOpt = s.option(form.ListValue, 'security', wlocI18n.t('Security'));
		securityOpt.value('', wlocI18n.t('None')); securityOpt.value('tls'); securityOpt.value('reality');
		securityOpt.depends('protocol', 'vless');
		securityOpt.depends('protocol', 'vmess');
		securityOpt.modalonly = true;
		// The compiler has no reality arm for VMess; selecting it would emit a
		// cleartext outbound that sing-box check accepts. Reject it up front.
		securityOpt.validate = function(section_id, value) {
			// LuCI 26 removed form.Map#getSectionValue; read the committed
			// protocol via the global uci API instead.
			if (value == 'reality' && uci.get('wificalling-gateway', section_id, 'protocol') == 'vmess')
				return false;
			return true;
		};
		var insecureOpt = s.option(form.Flag, 'insecure', wlocI18n.t('Allow insecure certificate'));
		insecureOpt.modalonly = true;
		var alpnOpt = s.option(form.Value, 'alpn', wlocI18n.t('ALPN'));
		alpnOpt.modalonly = true;
		var pinOpt = s.option(form.Value, 'pin_sha256', wlocI18n.t('TLS public-key SHA-256 (base64)'));
		pinOpt.modalonly = true;
		var flowOpt = s.option(form.Value, 'flow', wlocI18n.t('VLESS flow'));
		flowOpt.modalonly = true;
		var pubKeyOpt = s.option(form.Value, 'public_key', wlocI18n.t('Reality public key'));
		pubKeyOpt.modalonly = true;
		var shortIdOpt = s.option(form.Value, 'short_id', wlocI18n.t('Reality short ID'));
		shortIdOpt.modalonly = true;
		var fpOpt = s.option(form.Value, 'fingerprint', wlocI18n.t('Reality fingerprint'));
		fpOpt.modalonly = true;
		var udpMode = s.option(form.ListValue, 'udp_mode', wlocI18n.t('TUIC UDP mode'));
		udpMode.value('native', wlocI18n.t('Native')); udpMode.value('quic', wlocI18n.t('QUIC'));
		udpMode.modalonly = true;
		var transport = s.option(form.ListValue, 'transport', wlocI18n.t('Transport'));
		transport.value('', wlocI18n.t('None')); transport.value('ws', wlocI18n.t('WebSocket'));
		transport.modalonly = true;
		var pathOpt = s.option(form.Value, 'path', wlocI18n.t('WebSocket path'));
		pathOpt.modalonly = true;
		var hostOpt = s.option(form.Value, 'host', wlocI18n.t('WebSocket Host'));
		hostOpt.modalonly = true;
		var wgKey = s.option(form.Value, 'private_key', wlocI18n.t('WireGuard private key'));
		wgKey.password = true; wgKey.textvalue = function(id) { return this.cfgvalue(id) ? wlocI18n.t('Set') : wlocI18n.t('Not set'); };
		wgKey.modalonly = true;
		var localAddrOpt = s.option(form.Value, 'local_address', wlocI18n.t('WireGuard local address'));
		localAddrOpt.modalonly = true;
		var reservedOpt = s.option(form.Value, 'reserved', wlocI18n.t('WireGuard reserved (comma-separated)'));
		reservedOpt.modalonly = true;
		var mtuOpt = s.option(form.Value, 'mtu', wlocI18n.t('WireGuard MTU'));
		mtuOpt.modalonly = true;
		var wgPsk = s.option(form.Value, 'pre_shared_key', wlocI18n.t('WireGuard preshared key'));
		wgPsk.password = true; wgPsk.depends('protocol', 'wireguard');
		wgPsk.textvalue = function(id) { return this.cfgvalue(id) ? wlocI18n.t('Set') : wlocI18n.t('Not set'); };
		// The per-row connection test goes before the Edit/Delete buttons.
		var nodeRowActions = s.renderRowActions;
		s.renderRowActions = function(section_id, more_label, trEl) {
			var tdEl = nodeRowActions.call(this, section_id, more_label, trEl);
			if (!tdEl.lastElementChild) return tdEl;
			var testBtn = E('button', {
				'class': 'btn cbi-button cbi-button-action',
				id: 'wfc-node-test-' + section_id,
				title: wlocI18n.t('Run a fresh connection test for this node'),
				click: function() { runNodeTest(section_id, this); }
			}, 'nodeTest');
			tdEl.lastElementChild.insertBefore(testBtn, tdEl.lastElementChild.firstChild);
			return tdEl;
		};

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
		ips.datatype = 'ip4addr'; ips.rmempty = false; ips.placeholder = lanSubnetHint();
		// Pick a connected LAN device to fill in its real name and IP in
		// the add/edit modal (the modal inputs use the cbid DOM ids).
		var devicePicker = s.option(form.DummyValue, '_device_picker', wlocI18n.t('From connected devices'));
		devicePicker.rmempty = true;
		devicePicker.textvalue = function() { return ''; };
		devicePicker.renderWidget = function(section_id, option_index, cfgvalue) {
			if (!detectedDevices.length)
				return E('em', {}, wlocI18n.t('No connected devices detected.'));
			var select = E('select', { class: 'cbi-input-select' }, detectedDevices.map(function(d) {
				return E('option', { value: d.ip }, (d.name ? d.name + ' (' + d.ip + ')' : d.ip));
			}));
			select.addEventListener('change', function() {
				var ip = select.value;
				if (!ip) return;
				var labelInput = document.getElementById('widget.cbid.wificalling-gateway.' + section_id + '.label');
				if (labelInput) {
					var dev = detectedDevices.find(function(d) { return d.ip === ip; });
					labelInput.value = (dev && dev.name) ? dev.name : '';
					labelInput.dispatchEvent(new Event('input', { bubbles: true }));
				}
				var dynlist = document.getElementById('cbid.wificalling-gateway.' + section_id + '.source_ip');
				if (dynlist) {
					var existing = Array.prototype.map.call(
						dynlist.querySelectorAll('.item input[type=hidden]'),
						function(input) { return input.value; });
					if (existing.indexOf(ip) < 0) {
						var ipInput = document.getElementById('widget.cbid.wificalling-gateway.' + section_id + '.source_ip');
						if (ipInput) {
							ipInput.value = ip;
							ipInput.dispatchEvent(new Event('input', { bubbles: true }));
							var addBtn = dynlist.querySelector('.add-item .cbi-button-add');
							if (addBtn) addBtn.click();
						}
					}
				}
			});
			return select;
		};
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
			return L.resolveDefault(fetch('/wloc-node-status.json').then(function(r) { return r.text(); }), '{}').then(function(raw) {
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
