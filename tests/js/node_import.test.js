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
	console.log('VLESS legacy Reality import tests passed');
}

main();
