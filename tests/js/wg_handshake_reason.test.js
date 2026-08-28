'use strict';

// Regression guards for single-process node probing:
//
// 1. Node health and manual tests must use the loopback inbound of the
//    existing Gateway sing-box, never launch a temporary process.
// 2. The compiler must expose a private, per-node test inbound routed to the
//    corresponding outbound.
// 3. The existing UI and translations remain present for older cached data.

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

	// The patch source must synchronize the complete single-process probe set.
	const patch = fs.readFileSync(path.join(root, 'scripts/openwrt/patch-wireguard-health.sh'), 'utf8');
	assert(patch.includes('node-health.sh'),
		'patch-wireguard-health.sh: the health helper must be synchronized');
	assert(patch.includes('compiler.sh'),
		'patch-wireguard-health.sh: the compiler must be synchronized with the helper');
	assert(!patch.includes('sing-box run'),
		'patch-wireguard-health.sh: the patch must not start a second sing-box');

	const compiler = fs.readFileSync(path.join(root, 'openwrt/files/usr/libexec/wificalling-gateway/compiler.sh'), 'utf8');
	assert(compiler.includes('127.0.0.1'),
		'compiler.sh: probe inbounds must be loopback-only');
	assert(compiler.includes('inbound'),
		'compiler.sh: probe routes must select by inbound tag');
	assert(compiler.includes('probe_port_by_id'),
		'compiler.sh: probe ports must be tied to node ids');

	// The compact-status patch must carry the reason into the public export.
	const compact = fs.readFileSync(path.join(root, 'scripts/openwrt/patch-node-status-compact.sh'), 'utf8');
	assert(compact.includes('reason\\":%s}'),
		'patch-node-status-compact.sh: the public node-status export must include the failure reason');

	overviewSources.forEach(function(relative) {
		const source = fs.readFileSync(path.join(root, relative), 'utf8');
		assert(source.includes("n.reason === 'config_missing'") || source.includes("wgFailReason(n.reason)"),
			`${relative}: the view must render the config_missing handshake reason`);
		assert(source.includes("n.reason === 'timeout'") || source.includes("reason === 'timeout'"),
			`${relative}: the view must render the timeout handshake reason`);
		assert(source.includes("n.reason === 'unreachable'") || source.includes("reason === 'unreachable'"),
			`${relative}: the view must render the unreachable handshake reason`);
		assert(source.includes("wlocI18n.t('Handshake failed')"),
			`${relative}: the failed-handshake label must stay localizable`);
		// Short status-cell labels keep the node table compact; the full
		// explanation lives in the tooltip and result banner.
		assert(source.includes("wlocI18n.t('Missing config')"),
			`${relative}: the config_missing status cell must use the short label`);
		assert(source.includes("wlocI18n.t('Timeout')"),
			`${relative}: the timeout status cell must use the short label`);
		assert(source.includes("wlocI18n.t('Unreachable')"),
			`${relative}: the unreachable status cell must use the short label`);
		assert(source.includes('function wgFailDetail'),
			`${relative}: the full failure explanation must be available for tooltip/banner`);
		assert(source.includes('title: detail'),
			`${relative}: the status cell must carry the full explanation as a tooltip`);
		// Manual per-node connection test button.
		assert(source.includes("method: 'node_test'"),
			`${relative}: the view must declare the node_test rpcd method`);
		assert(source.includes("'wfc-node-test-'"),
			`${relative}: every node row must carry a test button`);
		assert(source.includes("}, 'nodeTest')"),
			`${relative}: the test button label must be nodeTest`);
		assert(source.includes('renderRowActions'),
			`${relative}: the test button must be injected into the row actions (before Edit/Delete)`);
		assert(source.includes('modalonly = true'),
			`${relative}: the detail fields must stay hidden from the table (visible in the edit modal)`);
		assert(source.includes("'label', wlocI18n.t('Name')"),
			`${relative}: the name column must be labelled Name`);
		assert(source.includes("nodeLabel.modalonly = true"),
			`${relative}: the label field must not duplicate the GridSection name column in the table`);
		// The test result banner must carry an explicit close button and
		// must not auto-dismiss (the stock notification's dismiss control
		// is easy to miss under some themes).
		assert(source.includes('function testNotify'),
			`${relative}: the test result banner must be rendered by a helper with an explicit close button`);
		assert(source.includes("wlocI18n.t('Close')"),
			`${relative}: the test result banner must have a close button`);
		// A successful manual test must update the row immediately; the
		// notification alone is not enough because the status export is polled
		// independently and may still contain the previous result.
		assert(source.includes('function updateNodeRow'),
			`${relative}: manual test results must have a row-update helper`);
		assert(source.includes('updateNodeRow(id, r);'),
			`${relative}: manual test results must refresh the node row`);
		assert(source.includes('manualNodeResults'),
			`${relative}: polling must not immediately overwrite a fresh manual result`);
		assert(source.includes('function nodeForDisplay'),
			`${relative}: manual results need an expiry before live status resumes`);
	});

	i18nSources.forEach(function(relative) {
		const source = fs.readFileSync(path.join(root, relative), 'utf8');
		assert(source.includes("'Missing key/address': '缺少私钥/公钥或本地地址'"),
			`${relative}: missing config_missing translation`);
		assert(source.includes("'Handshake timed out (key/psk mismatch?)': '服务器无响应，密钥或预共享密钥可能不匹配'"),
			`${relative}: missing timeout detail translation`);
		assert(source.includes("'Timeout': '超时'"),
			`${relative}: missing short timeout label`);
		assert(source.includes("'Unreachable': '不可达'"),
			`${relative}: missing short unreachable label`);
		assert(source.includes("'Server unreachable': '服务器不可达'"),
			`${relative}: missing unreachable translation`);
		assert(source.includes("'Close': '关闭'"),
			`${relative}: missing close button translation`);
		assert(source.includes("'Run a fresh connection test for this node'"),
			`${relative}: missing test button tooltip translation`);
		assert(source.includes("'Testing…': '测试中…'"),
			`${relative}: missing testing state translation`);
		assert(source.includes("'Unable to test node: '"),
			`${relative}: missing test failure translation`);
	});

	console.log('wireguard handshake reason guards passed');
}

main();
