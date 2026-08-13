'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');

function loadI18n(sourcePath, anchors, queuedFrames) {
	const source = fs.readFileSync(sourcePath, 'utf8');
	const document = {
		body: { className: 'lang_zh' },
		querySelectorAll: function() { return anchors; }
	};
	const window = {
		requestAnimationFrame: function(callback) {
			queuedFrames.push(callback);
		}
	};
	const baseclass = { extend: function(value) { return value; } };
	return new Function('baseclass', 'document', 'window', source)(
		baseclass, document, window
	);
}

function verifyDeferredTabLocalization(sourcePath) {
	const anchors = [];
	const queuedFrames = [];
	const i18n = loadI18n(sourcePath, anchors, queuedFrames);

	i18n.localizeTabs();
	anchors.push({ textContent: 'Help (FAQ)' });
	assert(queuedFrames.length > 0,
		'tab localization must retry after LuCI finishes replacing the tab bar');
	while (queuedFrames.length)
		queuedFrames.shift()();
	assert.strictEqual(anchors[0].textContent, '使用帮助（FAQ）');
}

function verifyFaqUsesLocalization(sourcePath) {
	const source = fs.readFileSync(sourcePath, 'utf8');
	assert(source.includes("'require wificalling-location-gateway.i18n as wlocI18n';"),
		`${sourcePath}: FAQ must load the shared tab localizer`);
	assert(source.includes('wlocI18n.localizeTabs();'),
		`${sourcePath}: FAQ must invoke the shared tab localizer`);
}

function main() {
	const root = path.resolve(__dirname, '..', '..');
	const i18nSources = [
		'openwrt/files/www/luci-static/resources/wificalling-location-gateway/i18n.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/wificalling-location-gateway/i18n.js'
	];
	const faqSources = [
		'openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/faq.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/faq.js'
	];
	i18nSources.forEach(function(relative) {
		verifyDeferredTabLocalization(path.join(root, relative));
	});
	faqSources.forEach(function(relative) {
		verifyFaqUsesLocalization(path.join(root, relative));
	});
	console.log('tab localization tests passed');
}

main();
