'use strict';

const assert = require('assert');
const fs = require('fs');
const path = require('path');

function find(node, predicate) {
	if (!node) return null;
	if (Array.isArray(node)) {
		for (const child of node) {
			const found = find(child, predicate);
			if (found) return found;
		}
		return null;
	}
	if (predicate(node)) return node;
	return find(node.children, predicate);
}

function loadNotify(sourcePath) {
	const source = fs.readFileSync(sourcePath, 'utf8');
	const start = source.indexOf('function notify(');
	const end = source.indexOf('\nfunction gpsOf', start);
	assert(start >= 0 && end > start, `${sourcePath}: notify helper not found`);

	const calls = { stock: 0 };
	const host = {
		children: [],
		firstElementChild: null,
		insertBefore(node) {
			node.parentNode = this;
			this.children.unshift(node);
			this.firstElementChild = this.children[0] || null;
		},
		removeChild(node) {
			this.children = this.children.filter((child) => child !== node);
			this.firstElementChild = this.children[0] || null;
			node.parentNode = null;
		}
	};
	function E(tag, attrs, children) {
		if (Array.isArray(tag)) {
			children = attrs;
			attrs = {};
			tag = 'fragment';
		}
		const node = { tag, attrs: attrs || {}, children: children == null ? [] : children, parentNode: null };
		if (!Array.isArray(node.children)) node.children = [node.children];
		return node;
	}
	const document = {
		body: host,
		querySelector() { return host; }
	};
	const window = { setTimeout() {} };
	const ui = { addNotification() { calls.stock += 1; } };
	const factory = new Function(
		'E', 'document', 'window', 'wlocI18n', 'ui',
		`${source.slice(start, end)}; return notify;`
	);
	return {
		notify: factory(E, document, window, { t: (value) => value }, ui),
		host,
		calls
	};
}

function main() {
	const root = path.resolve(__dirname, '..', '..');
	const sources = [
		'openwrt/files/www/luci-static/resources/view/wificalling-location-gateway/wloc.js',
		'openwrt/luci-app-wificalling-location-gateway/files/www/luci-static/resources/view/wificalling-location-gateway/wloc.js'
	];

	for (const relative of sources) {
		const loaded = loadNotify(path.join(root, relative));
		loaded.notify('Applied', 'Preset is now the active location.', 'success');
		assert.strictEqual(loaded.calls.stock, 0, `${relative}: success must not use stock LuCI notification`);
		assert.strictEqual(loaded.host.children.length, 0, `${relative}: success must not create a blocking banner`);

		const error = loaded.notify('Apply failed', 'daemon unavailable');
		assert.strictEqual(loaded.calls.stock, 0, `${relative}: errors must not use stock LuCI notification`);
		assert.strictEqual(loaded.host.children.length, 1, `${relative}: errors need one visible banner`);
		const close = find(error, (node) => node.tag === 'button' && node.attrs['aria-label'] === 'Close');
		assert(close, `${relative}: error banner needs an accessible close button`);
		close.attrs.click({
			preventDefault() {},
			stopPropagation() {}
		});
		assert.strictEqual(loaded.host.children.length, 0, `${relative}: close must remove the error banner`);
	}

	console.log('WLOC notification dismissal tests passed');
}

main();
