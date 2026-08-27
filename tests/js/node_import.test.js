'use strict';

// Regression guard for legacy/escaped VLESS Reality links commonly copied
// from subscription tools. The fixture is synthetic and contains no node
// credentials.
const assert = require('assert');
const fs = require('fs');
const path = require('path');

function loadParser() {
	const root = path.resolve(__dirname, '..', '..');
	const source = fs.readFileSync(path.join(root, 'openwrt/files/www/luci-static/resources/wificalling-gateway/node-import.js'), 'utf8');
	const baseclass = { extend: function(value) { return value; } };
	const translate = function(value) { return value; };
	return new Function('baseclass', '_', source)(baseclass, translate);
}

function main() {
	const parser = loadParser();
	const parsed = parser.parse(
		'vless\\://uuid@example.test:443?tls=1&peer=iosapps.example' +
		'&tfo=1&udp=3&xtls=2&pbk=public-key\\_value&sid=shortid&fingerprint=random'
	);

	assert.strictEqual(parsed.protocol, 'vless');
	assert.strictEqual(parsed.server, 'example.test');
	assert.strictEqual(parsed.port, '443');
	assert.strictEqual(parsed.sni, 'iosapps.example');
	assert.strictEqual(parsed.security, 'reality');
	assert.strictEqual(parsed.flow, 'xtls-rprx-vision');
	assert.strictEqual(parsed.public_key, 'public-key_value');
	assert.strictEqual(parsed.short_id, 'shortid');
	assert.strictEqual(parsed.fingerprint, 'random');

	const commonCases = [
		['anytls://user:secret@example.test:443?peer=example.test', { protocol: 'anytls', password: 'secret', sni: 'example.test' }],
		['hysteria2://user:secret@example.test:443?sni=example.test', { protocol: 'hysteria2', password: 'secret', sni: 'example.test' }],
		['tuic://uuid:secret@example.test:443?sni=example.test', { protocol: 'tuic', uuid: 'uuid', password: 'secret', sni: 'example.test' }],
		['trojan://user:secret@example.test:443?sni=example.test', { protocol: 'trojan', password: 'secret', sni: 'example.test' }],
		['vless://uuid@example.test:443?security=tls&sni=example.test', { protocol: 'vless', uuid: 'uuid', security: 'tls', sni: 'example.test' }],
		['AWG://peer-public@example.test:51820?private_key=private-key&local_address=10.0.0.2/32', { protocol: 'wireguard', public_key: 'peer-public', private_key: 'private-key', local_address: '10.0.0.2/32' }]
	];
	commonCases.forEach(function([uri, expected]) {
		const result = parser.parse(uri);
		Object.keys(expected).forEach(function(key) {
			assert.strictEqual(result[key], expected[key], uri + ': incorrect ' + key);
		});
	});

	const vmessPayload = Buffer.from(JSON.stringify({ add: 'example.test', port: 443, id: 'uuid', tls: 'tls', net: 'ws', host: 'example.test', path: '/' })).toString('base64');
	const vmess = parser.parse('VMESS://' + vmessPayload);
	assert.strictEqual(vmess.protocol, 'vmess');
	assert.strictEqual(vmess.transport, 'ws');
	assert.strictEqual(vmess.security, 'tls');

	const overviewSources = [
		'openwrt/files/www/luci-static/resources/view/wificalling-gateway/overview.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-gateway/overview.js'
	];
	overviewSources.forEach(function(relative) {
		const source = fs.readFileSync(path.join(path.resolve(__dirname, '..', '..'), relative), 'utf8');
		assert(/catch \(err\) \{ ui\.hideModal\(\); testNotify\(/.test(source), relative + ': import errors must close the modal before showing a notice');
		assert(source.includes("if (msg.parentNode) msg.parentNode.removeChild(msg);"), relative + ': notice close must remove the notice immediately');
	});
	console.log('VLESS legacy Reality import tests passed');
}

main();
