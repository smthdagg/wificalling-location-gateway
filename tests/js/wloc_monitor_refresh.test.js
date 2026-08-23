'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');

function textOf(node) {
	if (node == null) return '';
	if (typeof node === 'string') return node;
	if (Array.isArray(node)) return node.map(textOf).join('');
	return textOf(node.children);
}

function find(node, predicate) {
	if (node == null) return null;
	if (Array.isArray(node)) {
		for (const child of node) {
			const found = find(child, predicate);
			if (found) return found;
		}
		return null;
	}
	if (typeof node !== 'object') return null;
	if (predicate(node)) return node;
	return find(node.children, predicate);
}

function status(exitIp, error) {
	return {
		service_phase: 'intercepting',
		assigned_device: '192.168.31.175',
		geo_source: 'auto',
		exit: { state: exitIp ? 'verified' : 'unavailable', ip: exitIp, last_error: error || null },
		geo: { state: exitIp ? 'fresh' : 'unavailable' }
	};
}

async function runRefresh(sourcePath, nextStatus) {
	const source = fs.readFileSync(sourcePath, 'utf8');
	const calls = { ctl: 0, notifications: [] };
	const E = function(tag, attrs, children) {
		if (Array.isArray(tag) && children == null) {
			children = attrs;
			attrs = {};
		}
		return { tag, attrs: attrs || {}, children: children == null ? [] : children };
	};
	const rpc = { declare: function() {
		return function(method) {
			assert.strictEqual(method, 'refresh');
			calls.ctl += 1;
			return Promise.resolve({ result: {} });
		};
	} };
	const dom = { content: function(node, children) { node.children = children; } };
	const ui = {
		addNotification: function(_title, content, level) {
			calls.notifications.push({ level, text: textOf(content) });
		},
		showModal: function() {},
		hideModal: function() {}
	};
	const moduleFactory = new Function(
		'view', 'wlocI18n', 'fs', 'poll', 'dom', 'ui', 'uci', 'rpc', 'L', 'E',
		source
	);
	const page = moduleFactory(
		{ extend: function(value) { return value; } },
		{ t: function(value) { return value; }, localizeTabs: function() {} },
		{ read: function() { return Promise.resolve(JSON.stringify(nextStatus)); }, write: function() { return Promise.resolve(); } },
		{ add: function() {} },
		dom,
		ui,
		{ load: function() { return Promise.resolve(); }, sections: function() { return []; } },
		rpc,
		{ resolveDefault: function(value) { return value; } },
		E
	);
	const tree = page.render([JSON.stringify(status('203.0.113.10')), '', null]);
	const button = find(tree, function(node) {
		return node.tag === 'button' && textOf(node).indexOf('Refresh IP') >= 0;
	});
	assert(button, 'Refresh IP button not found');
	const result = button.attrs.click();
	assert(result && typeof result.then === 'function', 'refresh click must return its Promise');
	await result;
	return calls;
}

async function main() {
	const root = path.resolve(__dirname, '..', '..');
	const sources = [
		'openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-monitor.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/wloc-monitor.js'
	];
	for (const relative of sources) {
		const sourcePath = path.join(root, relative);
		const unchanged = await runRefresh(sourcePath, status('203.0.113.10'));
		assert.strictEqual(unchanged.ctl, 1);
		assert.deepStrictEqual(unchanged.notifications, [{
			level: 'info',
			text: 'IP refreshed; exit unchanged: 203.0.113.10'
		}]);

		const missing = await runRefresh(
			sourcePath,
			status(null, 'followed device node is missing; select and apply a WCG node')
		);
		assert.strictEqual(missing.ctl, 1);
		assert.deepStrictEqual(missing.notifications, [{
			level: 'error',
			text: 'IP refresh failed: followed device node is missing; select and apply a WCG node'
		}]);
	}
	console.log('WLOC monitor refresh feedback tests passed');
}

main().catch(function(error) {
	console.error(error.stack || error);
	process.exit(1);
});
