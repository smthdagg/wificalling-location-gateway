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
const monitorLoop = fs.readFileSync(
	path.join(root, 'openwrt/files/usr/libexec/wificalling-gateway/monitor-loop.sh'),
	'utf8'
);
const patch = fs.readFileSync(
	path.join(root, 'scripts/openwrt/patch-wireguard-health.sh'),
	'utf8'
);
const wlocProbe = fs.readFileSync(
	path.join(root, 'src/exitprobe/singbox.rs'),
	'utf8'
);

test('compiler gives every node a loopback probe routed to its existing outbound', () => {
	assert.match(compiler, /127\.0\.0\.1/);
	assert.match(compiler, /probe-/);
	assert.match(compiler, /inbound/);
	assert.match(compiler, /probe_port/);
});

test('node quality checks use ICMP without asking sing-box to proxy traffic', () => {
	assert.doesNotMatch(health, /curl|127\.0\.0\.1/);
	assert.doesNotMatch(nodeTest, /curl|127\.0\.0\.1/);
	assert.match(health, /ping/);
	assert.match(nodeTest, /ping/);
	assert.doesNotMatch(patch, /temporary sing-box/);
	assert.doesNotMatch(patch, /sing-box run/);
	assert.doesNotMatch(wlocProbe, /Command::new\(&self\.singbox_bin\)/);
	assert.match(wlocProbe, /existing_probe_port/);
});

test('background health work never overlaps, bursts, or repeatedly rewrites Passwall rules', () => {
	assert.match(health, /if kill -0 "\$lock_pid" 2>\/dev\/null; then\n\t\texit 0/);
	assert.doesNotMatch(monitorLoop, /passwall-bypass\.sh ensure/);
	assert.match(monitorLoop, /% 2/);
	assert.match(monitorLoop, /next_node\(\)/);
	assert.match(monitorLoop, /node-health\.sh "\$nodes" "\$node_output" "\$node"/);
});
