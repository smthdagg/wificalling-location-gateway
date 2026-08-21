'use strict';
'require view';
'require wificalling-location-gateway.i18n as wlocI18n';

var sections = [
  ['Getting started', [
    'Open Basic Settings and confirm the standalone WLOC service and provider configuration.',
    'Open Devices to create one profile per LAN device. Each profile owns one WLOC node, one device address, and its Auto or Manual location mode.',
    'Use Auto to follow the selected node exit. Use Manual to write latitude and longitude to the same device profile.',
    'Install the local WLOC CA profile only on the authorized test device, then enable trust for that certificate.'
  ]],
  ['Operations', [
    'Overview and Service Status show the daemon, provider, socket, redirect, and profile state.',
    'Logs & Monitoring shows bounded WLOC events. Raw WLOC responses, credentials, and precise device identifiers are not stored in support bundles.',
    'Component Update is an independent page. Stage a signed local package under /tmp/wloc-update and run preflight before applying it.',
    'The package checks this router architecture, OpenWrt release family, package format, free space, and WLOC API metadata before installation.'
  ]],
  ['Small-router guidance', [
    'Use sing-box tiny/lite or an existing PassWall sing-box executable. WLOC does not copy or manage a second full provider binary.',
    'Keep the bounded log and support-bundle limits enabled. Remove the old WLOC package before a clean AX6S installation when overlay storage is tight.',
    'If the provider is unavailable, WLOC stays fail-open and withdraws its redirect instead of inventing a location.'
  ]]
];

return view.extend({
  render: function() {
    wlocI18n.localizeTabs();
    return E([], [
      E('h2', {}, wlocI18n.t('Help (FAQ)')),
      E('p', {}, wlocI18n.t('Standalone WLOC operating guidance.')),
      sections.map(function(section) {
        return E('div', { class: 'cbi-section' }, [
          E('h3', {}, wlocI18n.t(section[0])),
          E('ul', {}, section[1].map(function(item) { return E('li', {}, wlocI18n.t(item)); }))
        ]);
      })
    ]);
  }
});
