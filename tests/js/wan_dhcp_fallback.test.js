'use strict';

// Regression guards for the secondary-router (no DHCP) fixes:
//
// 1. The device-policy status column must treat a device seen in the ARP
//    cache as online even when /tmp/dhcp.leases is empty (a router that
//    does not run DHCP, e.g. a secondary/AP router, has no leases at all).
// 2. The WLOC page must auto-(re)generate the iOS CA profile on load so the
//    /wloc-ca.mobileconfig link works without a manual "Regenerate profile"
//    click, and must surface the failure reason instead of a dead link.
// 3. The shared i18n table must carry the new status/error strings.

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
	const wlocSources = [
		'openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wloc.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/wloc.js'
	];
	const i18nSources = [
		'openwrt/files/www/luci-static/resources/wificalling-location-gateway/i18n.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/wificalling-location-gateway/i18n.js'
	];

	overviewSources.forEach(function(relative) {
		const source = fs.readFileSync(path.join(root, relative), 'utf8');
		assert(source.includes("if (arpDevices[ip]) return wlocI18n.t('Online (static IP)');"),
			`${relative}: ARP-cache devices must report online without a DHCP lease`);
		assert(source.includes("if (mac) return wlocI18n.t('Not bound yet');"),
			`${relative}: lease-bound devices must keep the bind status`);
		assert(source.includes("return wlocI18n.t('Device offline');"),
			`${relative}: offline must remain the last-resort state`);
		assert(source.includes("fetch('/wloc-node-status.json?t='"),
			`${relative}: node status must be read via the static docroot export`);
		assert(source.includes("cache: 'no-store'"),
			`${relative}: node status polling must bypass browser cache`);
	});

	wlocSources.forEach(function(relative) {
		const source = fs.readFileSync(path.join(root, relative), 'utf8');
		assert(source.includes('var autoRegen = regenProfile().then'),
			`${relative}: the page must auto-(re)generate the CA profile on load`);
		assert(source.includes('wlocI18n.t(\'Profile unavailable: \')'),
			`${relative}: a failed auto-regen must show the reason instead of a dead link`);
		assert(source.includes("'id': 'wloc-cert-link'"),
			`${relative}: the certificate link element must be identifiable`);
	});

	i18nSources.forEach(function(relative) {
		const source = fs.readFileSync(path.join(root, relative), 'utf8');
		assert(source.includes("'Online (static IP)': '在线（静态 IP）'"),
			`${relative}: missing Online (static IP) translation`);
		assert(source.includes("'Profile unavailable: ': '描述文件不可用：'"),
			`${relative}: missing Profile unavailable translation`);
	});

	console.log('secondary-router (no DHCP) fallback tests passed');
}

main();
