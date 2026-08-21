'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');

async function loadModeHandler(sourcePath, manualLat, manualLon, ctlResults) {
	const source = fs.readFileSync(sourcePath, 'utf8');
	const calls = { save: 0, apply: 0, restart: 0, ctl: [], notifications: [] };
	let modeOption;

	const section = {
		option: function(type, name) {
			const option = { value: function() {} };
			if (name === 'geo_source') modeOption = option;
			return option;
		}
	};
	const form = {
		Map: function() {
			this.section = function() { return section; };
			this.render = function() { return Promise.resolve({}); };
		},
		NamedSection: function() {},
		Flag: function() {},
		DummyValue: function() {},
		ListValue: function() {},
		Value: function() {}
	};
	const uci = {
		get: function(config, sectionName) {
			if (config === 'wloc-service' && sectionName === 'main') {
				return { manual_lat: manualLat, manual_lon: manualLon };
			}
			return null;
		},
		set: function() {},
		save: function() {
			calls.save += 1;
			return Promise.resolve();
		},
		sections: function() { return []; },
		delete: function() {},
		add: function() { return 'preset'; }
	};
	const ui = {
		changes: {
			apply: function() {
				calls.apply += 1;
				return applyGate.promise;
			}
		},
		addNotification: function(_node, content) { calls.notifications.push(content); },
		showModal: function() {},
		hideModal: function() {}
	};
	const rpc = {
		declare: function(spec) {
			if (spec.method === 'ctl') {
				return function(method, query, lat, lon) {
					calls.ctl.push([method, query, lat, lon]);
					var result = ctlResults && ctlResults.length
						? ctlResults.shift()
						: { result: {} };
					return Promise.resolve(result);
				};
			}
			if (spec.method === 'restart_unified') {
				return function() {
					calls.restart += 1;
					return Promise.resolve({ ok: true });
				};
			}
			return function() { return Promise.resolve({}); };
		}
	};
	const E = function() {
		return {
			appendChild: function() {},
			innerHTML: '',
			value: ''
		};
	};
	const view = { extend: function(value) { return value; } };
	// The page derives the CA profile URL from location.hostname; simulate
	// the browser environment (a typical LAN address) for the loader.
	const location = { hostname: '192.168.31.1' };
	const moduleFactory = new Function(
		'view', 'wlocI18n', 'form', 'fs', 'poll', 'uci', 'ui', 'rpc', 'L', 'E', 'location',
		source
	);
	const page = moduleFactory(
		view,
		{ t: function(value) { return value; }, localizeTabs: function() {} },
		form,
		{},
		{},
		uci,
		ui,
		rpc,
		{ resolveDefault: function(value) { return value; } },
		E,
		location
	);
	await page.render(['{}', '', null, null, {}, '{}']);
	assert(modeOption && typeof modeOption.onchange === 'function', 'location mode handler not found');
	return { handler: modeOption.onchange, calls };
}

async function verifyManualSwitch(sourcePath) {
	const harness = await loadModeHandler(sourcePath, '51.5074', '-0.1278');
	const result = harness.handler(null, 'main', 'manual');

	assert(result && typeof result.then === 'function', 'mode switch must return its Promise');
	await result;
	assert.deepStrictEqual(harness.calls.ctl, [
		['mode-set', 'manual', '51.5074', '-0.1278']
	], 'manual mode must use the atomic backend mode operation');
	assert.strictEqual(harness.calls.save, 0, 'the browser must not start a competing UCI apply');
	assert.strictEqual(harness.calls.apply, 0, 'the browser must not restart the service');
	assert.strictEqual(harness.calls.restart, 0, 'the browser must not restart the service');
	assert.strictEqual(harness.calls.notifications.length, 0);
}

async function verifyAutoSwitch(sourcePath) {
	const harness = await loadModeHandler(sourcePath, '51.5074', '-0.1278');
	const result = harness.handler(null, 'main', 'auto');
	await result;
	assert.deepStrictEqual(harness.calls.ctl, [
		['mode-set', 'auto', null, null]
	], 'auto mode must use the atomic backend mode operation');
	assert.strictEqual(harness.calls.restart, 0);
}

async function verifyManualSwitchWithoutCoordinates(sourcePath) {
	const harness = await loadModeHandler(sourcePath, '', '');
	const result = harness.handler(null, 'main', 'manual');
	assert(result && typeof result.then === 'function', 'rejected mode switch must return a Promise');
	assert.strictEqual(await result, false);
	assert.strictEqual(harness.calls.save, 0, 'invalid manual mode must not be persisted');
	assert.strictEqual(harness.calls.apply, 0, 'invalid manual mode must not be applied');
	assert.deepStrictEqual(harness.calls.ctl, [], 'invalid manual mode must not reach runtime control');
	assert.strictEqual(harness.calls.notifications.length, 1, 'the user must receive one actionable error');
}

async function main() {
	const root = path.resolve(__dirname, '..', '..');
	const sources = [
		'openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wloc.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/wloc.js'
	];
	for (const relative of sources) {
		const sourcePath = path.join(root, relative);
		await verifyManualSwitch(sourcePath);
		await verifyAutoSwitch(sourcePath);
		await verifyManualSwitchWithoutCoordinates(sourcePath);
	}
	console.log('wloc mode switch tests passed');
}

main().catch((error) => {
	console.error(error.stack || error);
	process.exit(1);
});
