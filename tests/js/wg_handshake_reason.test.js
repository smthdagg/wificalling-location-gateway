'use strict';

// Regression guards for the WireGuard handshake-failure diagnostics:
//
// 1. node-health.sh must emit a machine-readable reason for failed
//    handshakes (config_missing / timeout / unreachable) so the status
//    view can tell a bad node apart from a dead server.
// 2. The overview view must render that reason next to "Handshake
//    failed" instead of a bare "Offline".
// 3. The handshake probe must forward the reserved field (WARP-style
//    endpoints), derive its probe port with a busybox-safe hash, and
//    serialize concurrent monitor ticks so two runs cannot race on the
//    same probe port and hand each other the wrong exit IP.
// 4. The shared i18n table must carry the reason strings in both
//    languages.

const assert = require('assert');
const fs = require('fs');
const path = require('path');

function main() {
	const root = path.resolve(__dirname, '..', '..');
	const overviewSources = [
		'openwrt/files/www/luci-static/resources/view/wificalling-gateway/overview.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-gateway/overview.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/overview.js'
	];
	const i18nSources = [
		'openwrt/files/www/luci-static/resources/wificalling-location-gateway/i18n.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/wificalling-location-gateway/i18n.js'
	];

	// The patch source that generates node-health.sh's handshake test.
	const patch = fs.readFileSync(path.join(root, 'scripts/openwrt/patch-wireguard-health.sh'), 'utf8');
	assert(patch.includes("reason=config_missing") || patch.includes('config_missing'),
		'patch-wireguard-health.sh: missing-key handshakes must be classified as config_missing');
	assert(patch.includes("handshake did not complete"),
		'patch-wireguard-health.sh: unanswered handshakes must be classified as timeout');
	assert(patch.includes('reason=unreachable'),
		'patch-wireguard-health.sh: failed test launches must be classified as unreachable');
	assert(patch.includes('reserved=$(uci -q get'),
		'patch-wireguard-health.sh: the probe must forward the reserved field');
	assert(patch.includes("md5sum | cut -c1-4"),
		'patch-wireguard-health.sh: the probe port must use a busybox-safe hash (cksum is absent on ImmortalWrt)');
	assert(patch.includes('wg-health.lock'),
		'patch-wireguard-health.sh: concurrent monitor ticks must be serialized');
	assert(patch.includes('kill -0'),
		'patch-wireguard-health.sh: a lock left by a killed tick must be reclaimed');

	// The compact-status patch must carry the reason into the public export.
	const compact = fs.readFileSync(path.join(root, 'scripts/openwrt/patch-node-status-compact.sh'), 'utf8');
	assert(compact.includes('reason\\":%s}'),
		'patch-node-status-compact.sh: the public node-status export must include the failure reason');

	overviewSources.forEach(function(relative) {
		const source = fs.readFileSync(path.join(root, relative), 'utf8');
		assert(source.includes("n.reason === 'config_missing'"),
			`${relative}: the view must render the config_missing handshake reason`);
		assert(source.includes("n.reason === 'timeout'"),
			`${relative}: the view must render the timeout handshake reason`);
		assert(source.includes("n.reason === 'unreachable'"),
			`${relative}: the view must render the unreachable handshake reason`);
		assert(source.includes("wlocI18n.t('Handshake failed')"),
			`${relative}: the failed-handshake label must stay localizable`);
	});

	i18nSources.forEach(function(relative) {
		const source = fs.readFileSync(path.join(root, relative), 'utf8');
		assert(source.includes("'Missing key/address': '缺少私钥/公钥或本地地址'"),
			`${relative}: missing config_missing translation`);
		assert(source.includes("'Handshake timed out (key/psk mismatch?)'"),
			`${relative}: missing timeout translation key`);
		assert(source.includes("'Server unreachable': '服务器不可达'"),
			`${relative}: missing unreachable translation`);
	});

	console.log('wireguard handshake reason guards passed');
}

main();
