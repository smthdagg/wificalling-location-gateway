'use strict';
const assert = require('assert');
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..', '..');
for (const prefix of ['openwrt/files', 'openwrt/luci-app-wificalling-location-gateway/files']) {
  const health = fs.readFileSync(path.join(root, prefix, 'www/luci-static/resources/view/wificalling-location-gateway/wloc-health.js'), 'utf8');
  const update = fs.readFileSync(path.join(root, prefix, 'www/luci-static/resources/view/wificalling-location-gateway/wloc-update.js'), 'utf8');
  assert(health.includes("services.provider"), prefix + ': health page must use provider status');
  assert(update.includes("update_preflight"), prefix + ': component update must be separate');
  assert(!health.includes('update_apply'), prefix + ': health page must not apply updates');
  assert(!health.includes('services.gateway'), prefix + ': health page must not expose Gateway state');
}
console.log('standalone WLOC status/update contract passed');
