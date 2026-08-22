'use strict';
const assert = require('assert');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..', '..');
for (const prefix of ['openwrt/files', 'openwrt/luci-app-wificalling-location-gateway/files']) {
  const rpc = fs.readFileSync(path.join(root, prefix, 'usr/libexec/rpcd/luci.wloc'), 'utf8');
  const devices = fs.readFileSync(path.join(root, prefix, 'www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js'), 'utf8');
  assert(rpc.includes('mode-set'), prefix + ': runtime mode control must remain available');
  assert(devices.includes("['auto', wlocI18n.t('Auto follow selected node')]"), prefix + ': auto mode must be explicit');
  assert(devices.includes("['manual', wlocI18n.t('Manual location')]"), prefix + ': manual mode must be explicit');
  assert(rpc.includes('node_test'), prefix + ': integrated runtime must expose Gateway node diagnostics');
}
console.log('integrated WLOC device location mode contract passed');
