'use strict';
'require view';
'require rpc';
'require ui';
'require wificalling-location-gateway.i18n as i18n';

var statusRpc = rpc.declare({ object: 'luci.wloc', method: 'update_status' });
var preflightRpc = rpc.declare({ object: 'luci.wloc', method: 'update_preflight', params: ['path'] });
var applyRpc = rpc.declare({ object: 'luci.wloc', method: 'update_apply', params: ['path'] });
var recoverRpc = rpc.declare({ object: 'luci.wloc', method: 'update_recover' });

return view.extend({
  load: function() { return L.resolveDefault(statusRpc(), {}); },
  render: function(status) {
    i18n.localizeTabs();
    var path = E('input', { class: 'cbi-input-text', style: 'min-width:360px', placeholder: '/tmp/wloc-update/package.ipk' });
    var report = E('pre', { style: 'white-space:pre-wrap' }, JSON.stringify(status || {}, null, 2));
    function show(result) { report.textContent = JSON.stringify(result || {}, null, 2); }
    function action(call) {
      return function() {
        this.disabled = true;
        call(path.value).then(show).catch(function(error) {
          show({ error: String(error) });
          ui.addNotification(null, E('p', {}, i18n.t('Component update failed')), 'error');
        }).finally(function() { this.disabled = false; }.bind(this));
      };
    }
    return E([], [E('h2', {}, i18n.t('Component Update')), E('p', {}, i18n.t('Stage a signed local package under /tmp/wloc-update. Preflight checks this router before any installation.')), E('div', { class: 'cbi-section' }, [
      E('div', { class: 'cbi-value' }, [E('label', { class: 'cbi-value-title' }, i18n.t('Package path')), E('div', { class: 'cbi-value-field' }, path)]),
      E('p', {}, [
        E('button', { class: 'cbi-button', click: action(preflightRpc) }, i18n.t('Check package')), ' ',
        E('button', { class: 'cbi-button cbi-button-apply', click: action(applyRpc) }, i18n.t('Apply update')), ' ',
        E('button', { class: 'cbi-button', click: function() { recoverRpc().then(show).catch(function(e) { show({ error: String(e) }); }); } }, i18n.t('Recover interrupted update'))
      ]),
      report
    ])]);
  }
});

