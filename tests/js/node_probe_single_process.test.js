const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const root = path.resolve(__dirname, '..', '..');
const compiler = fs.readFileSync(
	path.join(root, 'openwrt/files/usr/libexec/wificalling-gateway/compiler.sh'),
	'utf8'
);
const health = fs.readFileSync(
	path.join(root, 'openwrt/files/usr/libexec/wificalling-gateway/node-health.sh'),
	'utf8'
);
const nodeTest = fs.readFileSync(
	path.join(root, 'openwrt/files/usr/libexec/wificalling-gateway/node-test.sh'),
	'utf8'
);
const patch = fs.readFileSync(
	path.join(root, 'scripts/openwrt/patch-wireguard-health.sh'),
	'utf8'
);

test('compiler gives every node a loopback probe routed to its existing outbound', () => {
	assert.match(compiler, /127\.0\.0\.1/);
	assert.match(compiler, /probe-/);
	assert.match(compiler, /inbound/);
	assert.match(compiler, /probe_port/);
});

test('node health and manual tests never launch a second sing-box', () => {
	assert.doesNotMatch(health, /sing-box run/);
	assert.doesNotMatch(nodeTest, /sing-box run/);
	assert.match(health, /127\.0\.0\.1/);
	assert.match(nodeTest, /127\.0\.0\.1/);
	assert.doesNotMatch(patch, /temporary sing-box/);
	assert.doesNotMatch(patch, /sing-box run/);
});
