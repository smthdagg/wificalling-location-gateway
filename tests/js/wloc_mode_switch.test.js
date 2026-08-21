'use strict';
const assert = require('assert');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..', '..');
for (const prefix of ['openwrt/files', 'openwrt/luci-app-wificalling-location-gateway/files']) {
  const rpc = fs.readFileSync(path.join(root, prefix, 'usr/libexec/rpcd/luci.wloc'), 'utf8');
  const devices = fs.readFileSync(path.join(root, prefix, 'www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js'), 'utf8');
  assert(rpc.includes('mode-set'), prefix + ': runtime mode control must remain available');
  assert(devices.includes("['auto', 'Auto follow selected node']"), prefix + ': auto mode must be explicit');
  assert(devices.includes("['manual', 'Manual location']"), prefix + ': manual mode must be explicit');
  assert(!rpc.includes('wificalling-gateway/node-test.sh'), prefix + ': runtime must not call Gateway helpers');
}
console.log('standalone WLOC device location mode contract passed');

