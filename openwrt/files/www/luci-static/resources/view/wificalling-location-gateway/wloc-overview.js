'use strict';
'require view';
'require rpc';
'require poll';
'require ui';
'require wificalling-location-gateway.i18n as i18n';

var getHealth = rpc.declare({ object: 'luci.wloc', method: 'health' });

return view.extend({
  load: function() { return L.resolveDefault(getHealth(), {}); },
  render: function(health) {
    i18n.localizeTabs();
    var body = E('div', {});
    function draw(current) {
      current = current || {};
      var services = current.services || {}, wloc = services.wloc || {}, provider = services.provider || {}, redirect = services.redirect || {};
      body.innerHTML = '';
      [[i18n.t('WLOC daemon'), wloc.running ? i18n.t('Running') : i18n.t('Stopped')],
       [i18n.t('Control socket'), wloc.socket ? i18n.t('Ready') : i18n.t('Missing')],
       [i18n.t('Provider'), provider.valid ? i18n.t('Ready') : i18n.t('Unavailable')],
       [i18n.t('Redirect table'), redirect.table_present ? i18n.t('Present') : i18n.t('Absent')],
       [i18n.t('Phase'), wloc.phase || i18n.t('Unknown')]].forEach(function(row) {
        body.appendChild(E('div', { class: 'cbi-value' }, [
          E('label', { class: 'cbi-value-title' }, row[0]),
          E('div', { class: 'cbi-value-field' }, row[1])
        ]));
      });
    }
    draw(health);
    poll.add(function() { return L.resolveDefault(getHealth(), {}).then(draw); }, 10);
    return E([], [E('h2', {}, i18n.t('WLOC Location Service')), E('p', {}, i18n.t('Observe the standalone WLOC service, provider, redirect scope, and device profiles.')), E('div', { class: 'cbi-section' }, [E('h3', {}, i18n.t('Current status')), body])]);
  }
});

