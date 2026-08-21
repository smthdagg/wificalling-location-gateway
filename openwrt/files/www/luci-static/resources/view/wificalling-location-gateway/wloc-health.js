'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require poll';
'require dom';
'require rpc';
'require ui';

// 服务状态页：wloc-service 与 Wi-Fi Calling Gateway（monitor-loop +
// sing-box）的进程、配置、规则、补丁、节点健康，以及两个服务的重启按钮。
// 数据由 /usr/sbin/wloc-health.sh 通过 luci.wloc rpcd `health` 方法提供。

var getHealth = rpc.declare({
	object: 'luci.wloc',
	method: 'health'
});

var restartUnified = rpc.declare({
	object: 'luci.wloc',
	method: 'restart_unified'
});

var createSupportBundle = rpc.declare({
	object: 'luci.wloc',
	method: 'support_bundle'
});

var getUpdateStatus = rpc.declare({ object: 'luci.wloc', method: 'update_status' });
var preflightUpdate = rpc.declare({ object: 'luci.wloc', method: 'update_preflight', params: [ 'path' ] });
var applyUpdate = rpc.declare({ object: 'luci.wloc', method: 'update_apply', params: [ 'path' ] });
var recoverUpdate = rpc.declare({ object: 'luci.wloc', method: 'update_recover' });

function notify(title, message, kind) {
	ui.addNotification(null, E('p', [ E('strong', title + ': '), message ]), kind);
}

// Compact status: a small colored dot plus short text.
function statusDot(ok, text) {
	return E('span', { 'class': 'wloc-health-dot' }, [
		E('span', {
			'class': 'dot',
			style: 'display:inline-block;width:8px;height:8px;border-radius:50%;' +
				(ok ? 'background:#16a34a;' : 'background:#dc2626;') +
				'margin-right:6px;vertical-align:middle'
		}),
		E('span', { style: 'vertical-align:middle' }, text)
	]);
}

return view.extend({
	load: function() {
		return L.resolveDefault(getHealth(), { error: wlocI18n.t('Health check unavailable') });
	},

	render: function(data) {
		wlocI18n.localizeTabs();
		var health = data || {};

		var wlocBody = E('div', {}, []);
		var gwBody = E('div', {}, []);
		var extraBody = E('div', {}, []);
		var profileBody = E('tbody', {}, []);
		var updateBody = E('div', {}, []);
		var updatePath = E('input', { 'class': 'cbi-input-text', 'style': 'min-width:360px', 'placeholder': '/tmp/wloc-update/package.ipk' });

		function renderUpdateStatus(status) {
			updateBody.innerHTML = '';
			status = status || {};
			var text = (status.phase || '-') + ' / ' + (status.reason || '-');
			if (status.current_version || status.target_version)
				text += ' (' + (status.current_version || '-') + ' → ' + (status.target_version || '-') + ')';
			updateBody.appendChild(statusDot(status.phase === 'applied', text));
		}

		function renderHealth(h) {
			wlocBody.innerHTML = '';
			gwBody.innerHTML = '';
			extraBody.innerHTML = '';
			profileBody.innerHTML = '';

			var s = h.services || {};
			var w = s.wloc || {};
			var g = s.gateway || {};
			var nodes = h.nodes || {};
			var patches = g.patches || {};

			function row(tbody, label, value) {
				tbody.appendChild(E('div', { style: 'padding:2px 0' }, [
					E('span', { style: 'display:inline-block;width:96px;color:#666;vertical-align:middle' }, label),
					E('span', { style: 'vertical-align:middle' }, value)
				]));
			}

			// wloc-service
			row(wlocBody, wlocI18n.t('Daemon'), statusDot(!!w.running, w.running ? wlocI18n.t('Running') : wlocI18n.t('Stopped')));
			row(wlocBody, wlocI18n.t('Socket'), yesNo(!!w.socket));
			row(wlocBody, wlocI18n.t('Status file'), statusDot(!!w.status_fresh, w.status_fresh ? wlocI18n.t('Fresh') : wlocI18n.t('Stale')));
			row(wlocBody, wlocI18n.t('Phase'), w.phase || '-');
			row(wlocBody, wlocI18n.t('Exit probe'), statusDot(w.exit === 'verified', w.exit || '-'));
			row(wlocBody, wlocI18n.t('Geo'), statusDot(w.geo === 'fresh', w.geo || '-'));

			// gateway
			row(gwBody, wlocI18n.t('Monitor'), statusDot(!!g.running, g.running ? wlocI18n.t('Running') : wlocI18n.t('Stopped')));
			row(gwBody, wlocI18n.t('sing-box'), statusDot(!!g.singbox, g.singbox ? wlocI18n.t('Running') : wlocI18n.t('Stopped')));
			row(gwBody, wlocI18n.t('Proxy config'), statusDot(!!g.config_valid, g.config_valid ? wlocI18n.t('Valid') : wlocI18n.t('Invalid')));
			row(gwBody, wlocI18n.t('Proxy config age'), ageText(g.config_age));
			if (g.config_stale) {
				row(gwBody, wlocI18n.t('Config changed'), E('span', { style: 'color:#d97706' },
					wlocI18n.t('Nodes/devices changed - restart the gateway to apply')));
			}
			row(gwBody, wlocI18n.t('nftables'), g.nft_rules + ' ' + wlocI18n.t('rules'));
			row(gwBody, wlocI18n.t('Devices'), g.devices + ' ' + wlocI18n.t('policies'));

			// patches + node health on one compact line
			function patchBadge(name, ok) {
				return E('span', { 'class': 'wloc-health-dot', style: 'margin-right:12px' }, [
					E('span', {
						'class': 'dot',
						style: 'display:inline-block;width:8px;height:8px;border-radius:50%;' +
							(ok ? 'background:#16a34a;' : 'background:#dc2626;') +
							'margin-right:4px;vertical-align:middle'
					}),
					E('span', { style: 'vertical-align:middle' }, name)
				]);
			}
			row(extraBody, wlocI18n.t('Patches'), E('span', {}, [
				patchBadge('PSK', !!patches.psk),
				patchBadge('WG ' + wlocI18n.t('handshake'), !!patches.handshake),
				patchBadge(wlocI18n.t('compact'), !!patches.compact),
				patchBadge(wlocI18n.t('guard'), !!patches.device_guard)
			]));
			row(extraBody, wlocI18n.t('Nodes'), E('span', {}, [
				statusDot(true, nodes.ok + '/' + nodes.total + ' ' + wlocI18n.t('online')),
				' ',
				nodes.down ? E('span', { style: 'color:#dc2626;margin-left:10px' }, nodes.down + ' ' + wlocI18n.t('offline')) : E([])
			]));
			if (h.error) {
				row(wlocBody, wlocI18n.t('Health check'), E('span', { style: 'color:#dc2626' }, h.error));
			}
			(h.profiles || []).forEach(function(profile) {
				var good = profile.phase === 'intercepting' || profile.phase === 'disabled';
				profileBody.appendChild(E('tr', { 'class': 'tr' }, [
					E('td', { 'class': 'td' }, profile.id),
					E('td', { 'class': 'td' }, profile.label || '-'),
					E('td', { 'class': 'td' }, statusDot(good, profile.phase || '-')),
					E('td', { 'class': 'td' }, profile.reason_code || '-')
				]));
			});
		}

		renderHealth(health);
		getUpdateStatus().then(renderUpdateStatus).catch(function() { renderUpdateStatus({ reason: wlocI18n.t('Update status unavailable') }); });

		poll.add(function() {
			return L.resolveDefault(getHealth(), { error: wlocI18n.t('Health check unavailable') }).then(function(h) {
				renderHealth(h);
				return getUpdateStatus().then(renderUpdateStatus).catch(function() {});
			});
		}, 15);

		// One-click service restarts, then refresh the report immediately.
		function restartAction(call, okText, busyLabel) {
			return function() {
				if (this.disabled) return;
				this.disabled = true;
				var original = this.textContent;
				this.textContent = busyLabel;
				call().then(function(r) {
					this.disabled = false;
					this.textContent = original;
					if (r && r.error) {
						notify(wlocI18n.t('Restart failed'), r.error, 'error');
						return;
					}
					notify(wlocI18n.t('Restarted'), okText, 'info');
					return L.resolveDefault(getHealth(), { error: wlocI18n.t('Health check unavailable') }).then(function(h) {
						renderHealth(h);
					});
				}.bind(this)).catch(function(e) {
					this.disabled = false;
					this.textContent = original;
					notify(wlocI18n.t('Restart failed'), String(e), 'error');
				}.bind(this));
			};
		}

		var supportButton = E('button', {
			'class': 'cbi-button cbi-button-apply',
			click: function() {
				if (this.disabled) return;
				this.disabled = true;
				createSupportBundle().then(function(result) {
					if (result && result.error) throw new Error(result.error);
					notify(wlocI18n.t('Support bundle ready'),
						(result && result.path ? result.path : '/tmp/wloc-support-bundle.tar.gz') +
						' (' + ((result && result.bytes) || '?') + ' bytes)', 'info');
				}).catch(function(error) {
					notify(wlocI18n.t('Support bundle failed'), String(error), 'error');
				}).then(function() { this.disabled = false; }.bind(this));
			}
		}, wlocI18n.t('Generate support bundle'));

		var restartButtons = E('div', { 'class': 'cbi-section', style: 'margin-top:16px' }, [
			E('h3', { style: 'margin-top:0' }, wlocI18n.t('Restart services')),
			E('div', {}, [
				E('button', {
					'class': 'cbi-button cbi-button-apply',
					'id': 'wloc-restart-gateway',
					style: 'margin-right:8px',
					click: restartAction(restartUnified,
						wlocI18n.t('Unified Gateway / WLOC service restarted'),
						wlocI18n.t('Restarting…'))
				}, wlocI18n.t('Restart unified service'))
			]),
			E('p', { style: 'color:#666;font-size:12px;margin-bottom:0' },
				wlocI18n.t('Restarting the gateway regenerates the proxy config and briefly interrupts device proxying.')),
			E('p', { style: 'color:#666;font-size:12px;margin-bottom:0' },
				wlocI18n.t('Support bundles contain bounded redacted diagnostics only; copy the generated file from /tmp before it expires.'), ' ', supportButton)
		]);

		function updateAction(call, successText) {
			return function() {
				if (this.disabled) return;
				this.disabled = true;
				call(updatePath.value).then(function(result) {
					if (result && result.error) throw new Error(result.error);
					notify(wlocI18n.t('Component update'), successText, 'info');
					return getUpdateStatus().then(renderUpdateStatus);
				}).catch(function(error) {
					notify(wlocI18n.t('Component update failed'), String(error), 'error');
				}).then(function() { this.disabled = false; }.bind(this));
			};
		}

		var updateSection = E('div', { 'class': 'cbi-section', style: 'margin-top:16px' }, [
			E('h3', { style: 'margin-top:0' }, wlocI18n.t('Component update')),
			E('p', {}, wlocI18n.t('Stage a validated IPK under /tmp/wloc-update before checking or applying it.')),
			E('div', {}, [updatePath, ' ', E('button', { 'class': 'cbi-button', click: updateAction(preflightUpdate, wlocI18n.t('Package preflight passed')) }, wlocI18n.t('Check package')), ' ', E('button', { 'class': 'cbi-button cbi-button-apply', click: updateAction(applyUpdate, wlocI18n.t('Update applied and health checked')) }, wlocI18n.t('Apply update')), ' ', E('button', { 'class': 'cbi-button', click: function() { recoverUpdate().then(function() { notify(wlocI18n.t('Component update'), wlocI18n.t('Interrupted transaction recovered'), 'info'); return getUpdateStatus().then(renderUpdateStatus); }).catch(function(e) { notify(wlocI18n.t('Component update failed'), String(e), 'error'); }); } }, wlocI18n.t('Recover'))]),
			E('div', { style: 'margin-top:8px' }, updateBody)
		]);

		return E([], [
			E('div', { 'class': 'cbi-section', style: 'margin-bottom:12px' }, [
				E('h3', { style: 'margin-top:0' }, wlocI18n.t('Gateway')),
				gwBody
			]),
			E('div', { 'class': 'cbi-section', style: 'margin-bottom:12px' }, [
				E('h3', { style: 'margin-top:0' }, wlocI18n.t('WLOC service')),
				wlocBody
			]),
			E('div', { 'class': 'cbi-section', style: 'margin-bottom:12px' }, [
				E('h3', { style: 'margin-top:0' }, wlocI18n.t('Patches and nodes')),
				extraBody
			]),
			E('div', { 'class': 'cbi-section', style: 'margin-bottom:12px' }, [
				E('h3', { style: 'margin-top:0' }, wlocI18n.t('Device profiles')),
				E('table', { 'class': 'table' }, [
					E('tr', { 'class': 'tr table-titles' }, [wlocI18n.t('ID'), wlocI18n.t('Label'), wlocI18n.t('State'), wlocI18n.t('Reason')].map(function(title) { return E('th', { 'class': 'th' }, title); })),
					profileBody
				])
			]),
			restartButtons,
			updateSection
		]);
	}
});

function yesNo(v) {
	return v ? '✓' : '✗';
}

function ageText(seconds) {
	if (seconds == null || seconds < 0) return '-';
	return wlocI18n.t('generated %d s ago').format(seconds);
}
