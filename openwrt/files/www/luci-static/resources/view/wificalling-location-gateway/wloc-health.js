'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';
'require poll';
'require dom';
'require rpc';

// 服务状态与日志（合并页）：wloc-service 与 Wi-Fi Calling Gateway
// （monitor-loop + sing-box）的进程、配置、规则、补丁、节点健康与最近日志。
// 数据由 /usr/sbin/wloc-health.sh 通过 luci.wloc rpcd `health` 方法提供。

var getHealth = rpc.declare({
	object: 'luci.wloc',
	method: 'health'
});

function yesNo(v) {
	return v ? wlocI18n.t('Yes') : wlocI18n.t('No');
}

function stateBadge(ok, text) {
	return E('span', { 'class': ok ? 'alert-message success' : 'alert-message error' }, text);
}

function ageText(seconds) {
	if (seconds == null || seconds < 0) return '-';
	if (seconds <= 120) return wlocI18n.t('Fresh');
	return wlocI18n.t('Stale (%d s ago)').format(seconds);
}

return view.extend({
	load: function() {
		return L.resolveDefault(getHealth(), { error: wlocI18n.t('Health check unavailable') });
	},

	render: function(data) {
		wlocI18n.localizeTabs();
		var health = data || {};

		/* ---------- 服务状态 ---------- */
		var wlocBody = E('tbody', {}, []);
		var gwBody = E('tbody', {}, []);
		var patchBody = E('tbody', {}, []);
		var logBody = E('tbody', {}, []);

		function renderHealth(h) {
			wlocBody.innerHTML = '';
			gwBody.innerHTML = '';
			patchBody.innerHTML = '';
			logBody.innerHTML = '';

			var s = h.services || {};
			var w = s.wloc || {};
			var g = s.gateway || {};
			var nodes = h.nodes || {};
			var patches = g.patches || {};

			function row(tbody, label, value) {
				tbody.appendChild(E('tr', { 'class': 'tr' }, [
					E('td', { 'class': 'td' }, label),
					E('td', { 'class': 'td' }, value)
				]));
			}
			function patchRow(tbody, label, ok) {
				row(tbody, label, stateBadge(ok, ok ? wlocI18n.t('Installed') : wlocI18n.t('Missing')));
			}

			// wloc-service
			row(wlocBody, wlocI18n.t('Daemon process'),
				stateBadge(!!w.running, w.running ? wlocI18n.t('Running') : wlocI18n.t('Stopped')));
			row(wlocBody, wlocI18n.t('Control socket'), yesNo(!!w.socket));
			row(wlocBody, wlocI18n.t('Status file fresh'),
				stateBadge(!!w.status_fresh, w.status_fresh ? wlocI18n.t('Fresh') : wlocI18n.t('Stale')));
			row(wlocBody, wlocI18n.t('Service phase'), w.phase || '-');
			row(wlocBody, wlocI18n.t('Exit probe'), w.exit || '-');
			row(wlocBody, wlocI18n.t('Geo resolution'), w.geo || '-');
			if (w.last_error && w.last_error !== 'null') {
				row(wlocBody, wlocI18n.t('Last error'), E('span', { 'class': 'alert-message error' }, w.last_error));
			}

			// gateway
			row(gwBody, wlocI18n.t('Monitor loop'),
				stateBadge(!!g.running, g.running ? wlocI18n.t('Running') : wlocI18n.t('Stopped')));
			row(gwBody, wlocI18n.t('sing-box proxy'),
				stateBadge(!!g.singbox, g.singbox ? wlocI18n.t('Running') : wlocI18n.t('Stopped')));
			row(gwBody, wlocI18n.t('Proxy config'),
				stateBadge(!!g.config_present, g.config_present ? wlocI18n.t('Present') : wlocI18n.t('Missing')));
			row(gwBody, wlocI18n.t('Config valid'),
				stateBadge(!!g.config_valid, g.config_valid ? wlocI18n.t('Valid') : wlocI18n.t('Invalid')));
			row(gwBody, wlocI18n.t('Config age'), ageText(g.config_age));
			row(gwBody, wlocI18n.t('Normalized config fresh'),
				stateBadge(!!g.normalized_fresh, g.normalized_fresh ? wlocI18n.t('Fresh') : wlocI18n.t('Stale')));
			row(gwBody, wlocI18n.t('nftables rules'), String(g.nft_rules));
			row(gwBody, wlocI18n.t('Device policies'), String(g.devices));

			// patches
			patchRow(patchBody, wlocI18n.t('WireGuard pre-shared key'), !!patches.psk);
			patchRow(patchBody, wlocI18n.t('WireGuard handshake check'), !!patches.handshake);
			patchRow(patchBody, wlocI18n.t('Compact node status'), !!patches.compact);
			patchRow(patchBody, wlocI18n.t('Stale device guard'), !!patches.device_guard);

			// nodes
			row(patchBody, wlocI18n.t('Nodes total'), String(nodes.total));
			row(patchBody, wlocI18n.t('Nodes online'),
				E('span', { 'class': 'alert-message success' }, String(nodes.ok)));
			row(patchBody, wlocI18n.t('Nodes offline'),
				E('span', { 'class': 'alert-message ' + (nodes.down ? 'error' : 'success') }, String(nodes.down)));
			row(patchBody, wlocI18n.t('Nodes unknown'), String(nodes.unknown));

			// log
			(h.log || []).forEach(function(line) {
				logBody.appendChild(E('tr', { 'class': 'tr' }, [
					E('td', { 'class': 'td' }, E('code', {}, line))
				]));
			});
			if (!(h.log || []).length) {
				row(logBody, wlocI18n.t('Recent logs'), wlocI18n.t('No log lines yet'));
			}

			if (h.error) {
				row(wlocBody, wlocI18n.t('Health check'), E('span', { 'class': 'alert-message error' }, h.error));
			}
		}

		renderHealth(health);

		poll.add(function() {
			return L.resolveDefault(getHealth(), { error: wlocI18n.t('Health check unavailable') }).then(function(h) {
				renderHealth(h);
			});
		}, 10);

		function section(title, tbody) {
			return E('div', { 'class': 'cbi-section' }, [
				E('h3', {}, title),
				E('table', { 'class': 'cbi-section-table' }, tbody)
			]);
		}

		return E([], [
			section(wlocI18n.t('WLOC service'), wlocBody),
			section(wlocI18n.t('Wi-Fi Calling Gateway'), gwBody),
			section(wlocI18n.t('Patches and node health'), patchBody),
			section(wlocI18n.t('Recent logs'), logBody)
		]);
	}
});
