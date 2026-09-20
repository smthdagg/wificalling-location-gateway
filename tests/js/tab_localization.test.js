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

function verifyFaqCopiesMatch(faqSources, root) {
	const canonical = fs.readFileSync(path.join(root, faqSources[0]), 'utf8');
	faqSources.slice(1).forEach(function(relative) {
		assert.strictEqual(fs.readFileSync(path.join(root, relative), 'utf8'), canonical,
			`${relative}: packaged FAQ must match the source FAQ exactly`);
	});
}

function verifyViewCopiesMatch(viewSources, root) {
	const canonical = fs.readFileSync(path.join(root, viewSources[0]), 'utf8');
	viewSources.slice(1).forEach(function(relative) {
		assert.strictEqual(fs.readFileSync(path.join(root, relative), 'utf8'), canonical,
			`${relative}: packaged view must match the source view exactly`);
	});
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
	const monitorSources = [
		'openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wfc-monitor.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/wfc-monitor.js'
	];
	i18nSources.forEach(function(relative) {
		verifyDeferredTabLocalization(path.join(root, relative));
	});
	faqSources.forEach(function(relative) {
		verifyFaqUsesLocalization(path.join(root, relative));
	});
	verifyFaqCopiesMatch(faqSources, root);
	// The packaged monitor view is the luci-app overlay; a diverging copy in
	// openwrt/files once silently shipped an older single-ePDG renderer.
	verifyViewCopiesMatch(monitorSources, root);
	verifyViewCopiesMatch(i18nSources, root);
	console.log('tab localization tests passed');
}

main();
