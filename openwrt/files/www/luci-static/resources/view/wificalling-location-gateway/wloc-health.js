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
		var logBody = E('div', {}, []);

		function renderHealth(h) {
			wlocBody.innerHTML = '';
			gwBody.innerHTML = '';
			extraBody.innerHTML = '';
			logBody.innerHTML = '';

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
			row(gwBody, wlocI18n.t('Config age'), ageText(g.config_age));
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

			// log lines
			(h.log || []).forEach(function(line) {
				logBody.appendChild(E('div', { style: 'padding:1px 0' },
					E('code', { style: 'font-size:11px;color:#555;word-break:break-all' }, line)));
			});
			if (!(h.log || []).length) {
				row(logBody, wlocI18n.t('Recent logs'), wlocI18n.t('No log lines yet'));
			}
			if (h.error) {
				row(wlocBody, wlocI18n.t('Health check'), E('span', { style: 'color:#dc2626' }, h.error));
			}
		}

		renderHealth(health);

		poll.add(function() {
			return L.resolveDefault(getHealth(), { error: wlocI18n.t('Health check unavailable') }).then(function(h) {
				renderHealth(h);
			});
		}, 10);

		return E([], [
			E('div', { 'class': 'cbi-section', style: 'margin-bottom:12px' }, [
				E('h3', { style: 'margin-top:0' }, wlocI18n.t('WLOC service')),
				wlocBody
			]),
			E('div', { 'class': 'cbi-section', style: 'margin-bottom:12px' }, [
				E('h3', { style: 'margin-top:0' }, wlocI18n.t('Gateway')),
				gwBody
			]),
			E('div', { 'class': 'cbi-section', style: 'margin-bottom:12px' }, [
				E('h3', { style: 'margin-top:0' }, wlocI18n.t('Patches and nodes')),
				extraBody
			]),
			E('div', { 'class': 'cbi-section' }, [
				E('h3', { style: 'margin-top:0' }, wlocI18n.t('Recent logs')),
				logBody
			])
		]);
	}
});

function yesNo(v) {
	return v ? '✓' : '✗';
}

function ageText(seconds) {
	if (seconds == null || seconds < 0) return '-';
	if (seconds <= 120) return wlocI18n.t('Fresh');
	return wlocI18n.t('Stale (%d s)').format(seconds);
}
