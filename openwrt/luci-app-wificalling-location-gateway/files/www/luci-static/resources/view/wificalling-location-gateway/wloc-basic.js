'use strict';
'require view';
'require form';
'require uci';
'require rpc';
'require ui';
'require wificalling-location-gateway.i18n as i18n';

var regenProfile = rpc.declare({ object: 'luci.wloc', method: 'regen_profile' });

return view.extend({
  load: function() { return Promise.all([uci.load('wloc-service'), L.resolveDefault(regenProfile(), {})]); },
  render: function(data) {
    i18n.localizeTabs();
    var m = new form.Map('wloc-service', i18n.t('Basic Settings'),
      i18n.t('Global WLOC service defaults. Device-specific node and location settings are managed on the Devices page.'));
    var s = m.section(form.NamedSection, 'main', 'wloc-service');
    s.anonymous = true;
    s.option(form.Flag, 'enabled', i18n.t('Enable WLOC service'));
    var provider = s.option(form.ListValue, 'geo_provider', i18n.t('Geo provider'));
    provider.value('http', 'HTTP provider');
    provider.value('stub', 'Stub provider');
    var interval = s.option(form.Value, 'probe_interval', i18n.t('Probe interval (seconds)'));
    interval.datatype = 'range(30,86400)';
    interval.default = '300';
    var port = s.option(form.Value, 'probe_port', i18n.t('Probe port'));
    port.datatype = 'port';
    var config = s.option(form.Value, 'singbox_config', i18n.t('sing-box provider configuration'));
    config.description = i18n.t('Optional path to an existing provider configuration. This project does not manage another application configuration.');
    var profile = data[1] || {};
    var link = profile.url ? E('a', { href: profile.url, target: '_blank' }, profile.url) : i18n.t('Profile unavailable');
    return m.render().then(function(node) {
      return E([], [node, E('div', { class: 'cbi-section' }, [
        E('h3', {}, i18n.t('iPhone certificate profile')),
        E('p', {}, i18n.t('Install the WLOC CA only on the authorized test device.')),
        E('p', {}, link)
      ])]);
    });
  }
});

