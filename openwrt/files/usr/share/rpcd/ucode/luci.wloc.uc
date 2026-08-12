'use strict';

// luci.wloc — LuCI RPC backend for the WLOC Location module.
//
// Bridges the LuCI frontend to the root-only control plane:
//   * control actions go through /usr/sbin/wloc-ctl (framed UDS request)
//   * the CA profile is (re)generated with export-mobileconfig.sh
//
// Every method name is validated against a fixed whitelist; arguments are
// passed as argv (never through a shell), so the control plane stays
// injection-free. The daemon socket itself is never exposed to LuCI.

const child_process = require('child_process');
const fs = require('fs');

const CTL = '/usr/sbin/wloc-ctl';
const EXPORT_PROFILE = '/usr/sbin/export-mobileconfig.sh';

// method -> additional fixed argv
const METHODS = {
    'status': [],
    'enable': [],
    'disable': [],
    'reload': [],
    'geo-set': [],
    'geo-clear': [],
};

function run_ctl(method, args) {
    if (!(method in METHODS))
        return { error: 'unknown control method' };

    let argv = [ CTL, method ].concat(METHODS[method]).concat(args);
    let rv = child_process.exec(argv, { timeout: 30000 });

    if (!rv)
        return { error: 'failed to start wloc-ctl' };

    if (rv.code !== 0)
        return { error: rv.stderr ? rv.stderr.trim() : 'wloc-ctl failed' };

    try {
        let payload = JSON.parse(rv.stdout);
        if (payload.error)
            return { error: payload.error.code || 'daemon error' };
        return payload.result || {};
    }
    catch (e) {
        return { error: rv.stdout.trim() || 'unparseable daemon reply' };
    }
}

function ctl(method, query, lat, lon) {
    if (method !== 'geo-set')
        return run_ctl(method, []);

    let args = [];
    if (query !== null && query !== '')
        args = [ '--query', query ];
    else if (lat !== null && lon !== null)
        args = [ '--lat', lat, '--lon', lon ];
    else
        return { error: 'geo-set needs --query or --lat --lon' };

    return run_ctl(method, args);
}

// (Re)generate the iOS configuration profile served by uhttpd so the test
// device can install the wloc-service root CA through Safari.
function regen_profile() {
    if (!fs.stat(EXPORT_PROFILE))
        return { error: 'export-mobileconfig.sh not installed' };

    let rv = child_process.exec([ '/bin/sh', EXPORT_PROFILE ], { timeout: 15000 });
    if (!rv || rv.code !== 0)
        return { error: rv && rv.stderr ? rv.stderr.trim() : 'profile generation failed' };

    return { ok: true, url: 'http://192.168.31.1/wloc-ca.mobileconfig' };
}

return {
    ctl: ctl,
    regen_profile: regen_profile,
};
