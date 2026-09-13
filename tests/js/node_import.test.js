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

	const encodedAuthority = Buffer.from('auto:uuid@example.test:443').toString('base64').replace(/=+$/, '');
	const encodedAuthorityResult = parser.parse(
		'vless://' + encodedAuthority + '?tls=1&peer=example.test&xtls=2&pbk=public-key&sid=shortid'
	);
	assert.strictEqual(encodedAuthorityResult.uuid, 'uuid');
	assert.strictEqual(encodedAuthorityResult.server, 'example.test');
	assert.strictEqual(encodedAuthorityResult.port, '443');
	assert.strictEqual(encodedAuthorityResult.security, 'reality');

	const emptyPrefixAuthority = Buffer.from(':uuid@example.test:443').toString('base64').replace(/=+$/, '');
	const emptyPrefixResult = parser.parse(
		'vless://' + emptyPrefixAuthority + '?tls=1&peer=example.test&xtls=2&pbk=public-key&sid=shortid'
	);
	assert.strictEqual(emptyPrefixResult.uuid, 'uuid');
	assert.strictEqual(emptyPrefixResult.server, 'example.test');
	assert.strictEqual(emptyPrefixResult.port, '443');

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
		assert(/'trojan','wireguard','shadowsocks'/.test(source), relative + ': shadowsocks must be selectable as a protocol');
		assert(/methodOpt\.depends|protocol'\) != 'shadowsocks'/.test(source), relative + ': the shadowsocks cipher field must be protocol-scoped');
	});

	// Shadowsocks: SIP002 (base64 userinfo), legacy (fully encoded authority),
	// and cleartext userinfo all have to land on the same node.
	const ssUserinfo = Buffer.from('aes-256-gcm:secret').toString('base64').replace(/=+$/, '');
	const sip002 = parser.parse('ss://' + ssUserinfo + '@example.test:8388#' + encodeURIComponent('HK 01'));
	assert.strictEqual(sip002.protocol, 'shadowsocks');
	assert.strictEqual(sip002.server, 'example.test');
	assert.strictEqual(sip002.port, '8388');
	assert.strictEqual(sip002.method, 'aes-256-gcm');
	assert.strictEqual(sip002.password, 'secret');
	assert.strictEqual(sip002.label, 'HK 01');

	const legacyPayload = Buffer.from('aes-256-gcm:secret@example.test:8388').toString('base64').replace(/=+$/, '');
	const legacy = parser.parse('ss://' + legacyPayload + '#legacy');
	assert.strictEqual(legacy.method, 'aes-256-gcm');
	assert.strictEqual(legacy.password, 'secret');
	assert.strictEqual(legacy.server, 'example.test');
	assert.strictEqual(legacy.port, '8388');

	const cleartext = parser.parse('SS://aes-128-gcm:secret@example.test:8388');
	assert.strictEqual(cleartext.method, 'aes-128-gcm');
	assert.strictEqual(cleartext.label, 'SS example.test');

	// A password containing ':' must not be split at the wrong colon.
	const colonPw = Buffer.from('aes-256-gcm:se:cret').toString('base64').replace(/=+$/, '');
	assert.strictEqual(parser.parse('ss://' + colonPw + '@example.test:8388').password, 'se:cret');

	// Plugins change the wire format; importing one silently would produce a
	// node that looks configured and never connects.
	assert.throws(function() {
		parser.parse('ss://' + ssUserinfo + '@example.test:8388?plugin=obfs-local%3Bobfs%3Dhttp');
	}, /plugin/i, 'shadowsocks plugin links must be rejected');

	assert.throws(function() {
		parser.parse('ss://' + Buffer.from('aes-256-gcm:secret').toString('base64') + '@example.test');
	}, /Server and port/, 'a shadowsocks link without a port must be rejected');

	console.log('VLESS legacy Reality and Shadowsocks import tests passed');
}

main();
