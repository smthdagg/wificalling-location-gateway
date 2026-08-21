'use strict';
const assert = require('assert');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..', '..');
for (const prefix of ['openwrt/files', 'openwrt/luci-app-wificalling-location-gateway/files']) {
  const devices = fs.readFileSync(path.join(root, prefix, 'www/luci-static/resources/view/wificalling-location-gateway/wloc-devices.js'), 'utf8');
  const basic = fs.readFileSync(path.join(root, prefix, 'www/luci-static/resources/view/wificalling-location-gateway/wloc-basic.js'), 'utf8');
  assert(devices.includes('assigned_device'), prefix + ': device profiles must own their device address');
  assert(devices.includes('manual_lat') && devices.includes('manual_lon'), prefix + ': manual location must be profile-scoped');
  assert(basic.includes("form.Map('wloc-service'"), prefix + ': basic settings must use WLOC UCI');
}
console.log('standalone WLOC profile/basic settings contract passed');

