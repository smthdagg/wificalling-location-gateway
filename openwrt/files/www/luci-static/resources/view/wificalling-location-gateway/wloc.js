'use strict';
'require view';
'require form';
'require fs';
'require poll';
'require uci';
'require ui';
'require rpc';

// WLOC Location module of the Wi-Fi Calling & WLOC gateway.
//
// Eight features on one page:
//   1. Safari CA profile (link + regenerate)
//   2. Location mode: auto (follow node) / manual
//   3. Module on/off switch
//   4. Current geo info (country/city, read-only)
//   5. Full GPS values (read-only, admin-only file)
//   6. Manual search (Nominatim) and coordinate entry
//   7. Saved presets, one-tap apply
//   8. WLOC usage log (events.jsonl)

var callCtl = rpc.declare({
	object: 'luci.wloc',
	method: 'ctl',
	params: [ 'method', 'query', 'lat', 'lon' ]
});

var regenProfile = rpc.declare({
	object: 'luci.wloc',
	method: 'regen_profile'
});

var STATUS_FILE = '/var/run/wloc-service/status.json';
var EVENTS_FILE = '/var/run/wloc-service/events.jsonl';
var PROFILE_URL = 'http://192.168.31.1/wloc-ca.mobileconfig';

function fmtTime(unix) {
	if (!unix) return '-';
	return new Date(unix * 1000).toLocaleString();
}

function notify(title, message) {
	ui.addNotification(null, E('p', [ E('strong', title + ': '), message ]));
}

function gpsOf(geo) {
	return (geo && geo.latitude != null && geo.longitude != null)
		? geo.latitude.toFixed(6) + ' / ' + geo.longitude.toFixed(6) : '-';
}

return view.extend({
	load: function() {
		return Promise.all([
			L.resolveDefault(fs.read(STATUS_FILE), '{}'),
			L.resolveDefault(fs.read(EVENTS_FILE), ''),
			uci.load('wloc-service')
		]);
	},

	render: function(data) {
		var status;
		try { status = JSON.parse(data[0]); } catch (e) { status = {}; }
		var eventsText = data[1] || '';

		var m = new form.Map('wloc-service', _('WLOC 设置'),
			_('WLOC 定位拦截：改写 Apple WLOC 响应，使测试设备上报网关指定的位置。GPS 数值只保留在本路由器上。'));

		/* ---------- 3. Module on/off switch ---------- */
		var so = m.section(form.NamedSection, 'main', 'wloc-service');
		so.anonymous = true;
		so.title = _('模块');

		var enabled = so.option(form.Flag, 'enabled', _('启用 WLOC 拦截'),
			_('打开/关闭 WLOC 改写。nftables 重定向规则保留；关闭时 Apple WLOC 流量原样透传。'));
		enabled.onchange = function(ev, section_id, value) {
			var on = (value === true || value === 1 || value === '1');
			callCtl(on ? 'enable' : 'disable', null, null, null).then(function(r) {
				if (r.error) {
					notify(_('切换失败'), r.error);
					return;
				}
				uci.set('wloc-service', 'main', 'enabled', on ? '1' : '0');
				uci.save('wloc-service');
				ui.changes.apply(true);
			});
		};

		so.option(form.DummyValue, '_phase', _('服务阶段')).cfgvalue = function() {
			return status.service_phase || '-';
		};

		/* ---------- 2. Location mode: auto / manual ---------- */
		var mode = so.option(form.ListValue, 'geo_source', _('定位模式'),
			_('自动模式跟随测试设备绑定的 sing-box 节点出口；手动模式使用下方搜索或输入的坐标。'));
		mode.value('auto', _('自动（跟随节点）'));
		mode.value('manual', _('手动位置'));
		mode.onchange = function(ev, section_id, value) {
			var main = uci.get('wloc-service', 'main');
			uci.set('wloc-service', 'main', 'geo_source', value);
			uci.save('wloc-service');
			ui.changes.apply(true);
			if (value === 'auto') {
				callCtl('geo-clear', null, null, null).then(function(r) {
					if (r.error) notify(_('模式切换失败'), r.error);
				});
			}
			else if (main && main.manual_lat && main.manual_lon) {
				callCtl('geo-set', null, main.manual_lat, main.manual_lon).then(function(r) {
					if (r.error) notify(_('模式切换失败'), r.error);
				});
			}
		};

		so.option(form.Value, 'manual_lat', _('手动纬度'));
		so.option(form.Value, 'manual_lon', _('手动经度'));

		/* ---------- 6. Manual search + coordinate apply ---------- */
		var searchResult = E('div', { 'class': 'cbi-row', 'id': 'wloc-search-result' });
		var queryField = E('input', {
			'class': 'cbi-input-text',
			'id': 'wloc-search-query',
			'type': 'text',
			'placeholder': _('例如：伦敦')
		});
		var searchBtn = E('button', {
			'class': 'cbi-button cbi-button-apply',
			'id': 'wloc-search-btn',
			'click': function() {
				var q = document.getElementById('wloc-search-query').value.trim();
				if (!q) return;
				this.disabled = true;
				// 先搜索：只返回城市与坐标，不改变当前定位。
				callCtl('geo-search', q, null, null).then(function(r) {
					searchBtn.disabled = false;
					var found = r && r.result;
					if (r.error || !found || found.latitude == null || found.longitude == null) {
						notify(_('搜索失败'), r.error || _('未找到该地名'));
						return;
					}
					var city = found.city || q;
					var lat = String(Number(found.latitude).toFixed(6));
					var lon = String(Number(found.longitude).toFixed(6));
					document.getElementById('wloc-coord-lat').value = lat;
					document.getElementById('wloc-coord-lon').value = lon;
					searchResult.innerHTML = '';
					searchResult.appendChild(E('p', {}, _('搜索结果：') + city +
						_('（纬度 ') + lat + _('，经度 ') + lon + _('）') +
						_('，确认后点击「应用坐标」。')));
				});
			}
		}, _('搜索'));

		var coordLat = E('input', { 'class': 'cbi-input-text', 'id': 'wloc-coord-lat', 'type': 'text', 'placeholder': '51.5074' });
		var coordLon = E('input', { 'class': 'cbi-input-text', 'id': 'wloc-coord-lon', 'type': 'text', 'placeholder': '-0.1278' });
		var coordBtn = E('button', {
			'class': 'cbi-button cbi-button-apply',
			'id': 'wloc-coord-btn',
			'click': function() {
				var lat = document.getElementById('wloc-coord-lat').value.trim();
				var lon = document.getElementById('wloc-coord-lon').value.trim();
				if (!lat || !lon) return;
				this.disabled = true;
				callCtl('geo-set', null, lat, lon).then(function(r) {
					coordBtn.disabled = false;
					if (r.error) {
						notify(_('应用失败'), r.error);
						return;
					}
					uci.set('wloc-service', 'main', 'manual_lat', lat);
					uci.set('wloc-service', 'main', 'manual_lon', lon);
					uci.set('wloc-service', 'main', 'geo_source', 'manual');
					uci.save('wloc-service');
					ui.changes.apply(true);
					notify(_('已应用'), _('坐标已成为当前生效位置。'));
				});
			}
		}, _('应用坐标'));

		var searchBox = E('div', { 'class': 'cbi-section' }, [
			E('h3', {}, _('手动搜索')),
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' }, [
					E('label', { 'for': 'wloc-search-query', 'class': 'cbi-value-title' },
						[ _('地名'), ' ', _('（在线搜索）') ]),
					E('div', { 'class': 'cbi-value-field' }, [ queryField, ' ', searchBtn ])
				])),
			searchResult,
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' }, [
					E('label', { 'class': 'cbi-value-title' }, _('或直接输入坐标')),
					E('div', { 'class': 'cbi-value-field' }, [ coordLat, ' ', coordLon, ' ', coordBtn ])
				]))
		]);

		/* ---------- 7. Presets (self-drawn table) ---------- */
		var presetBody = E('tbody', { 'id': 'wloc-preset-body' });
		var presetTable = E('table', { 'class': 'cbi-section-table' }, [
			E('thead', {},
				E('tr', {}, [
					E('th', {}, _('名称')),
					E('th', {}, _('纬度')),
					E('th', {}, _('经度')),
					E('th', {}, '')
				])),
			presetBody
		]);

		function applyPreset(sid) {
			var s = uci.get('wloc-service', sid);
			if (!s || !s.latitude || !s.longitude) {
				notify(_('应用失败'), _('预设没有坐标。'));
				return;
			}
			uci.set('wloc-service', 'main', 'manual_lat', s.latitude);
			uci.set('wloc-service', 'main', 'manual_lon', s.longitude);
			uci.set('wloc-service', 'main', 'geo_source', 'manual');
			uci.save('wloc-service').then(function() {
				return ui.changes.apply(true);
			}).then(function() {
				return callCtl('geo-set', null, s.latitude, s.longitude);
			}).then(function(r) {
				if (r && r.error) notify(_('应用失败'), r.error);
				else notify(_('已应用'), _('预设已成为当前生效位置。'));
			}).catch(function(e) {
				notify(_('应用失败'), String(e));
			});
		}

		function removePreset(sid) {
			uci.delete('wloc-service', sid);
			uci.save('wloc-service').then(function() {
				return ui.changes.apply(true);
			}).then(function() {
				renderPresets();
			}).catch(function(e) {
				notify(_('应用失败'), String(e));
			});
		}

		function renderPresets() {
			presetBody.innerHTML = '';
			var presets = uci.sections('wloc-service', 'preset');
			if (!presets.length) {
				presetBody.appendChild(E('tr', {}, [ E('td', { 'colspan': 4 }, _('暂无已保存的位置。')) ]));
				return;
			}
			presets.forEach(function(s) {
				var sid = s['.name'];
				presetBody.appendChild(E('tr', {}, [
					E('td', {}, s.label || sid),
					E('td', {}, s.latitude || '-'),
					E('td', {}, s.longitude || '-'),
					E('td', {}, [
						E('button', {
							'class': 'cbi-button cbi-button-apply',
							'click': function() { applyPreset(sid); }
						}, _('应用')),
						' ',
						E('button', {
							'class': 'cbi-button cbi-button-remove',
							'click': function() { removePreset(sid); }
						}, _('删除'))
					])
				]));
			});
		}
		renderPresets();

		var addPresetBtn = E('button', {
			'class': 'cbi-button cbi-button-add',
			'click': function() {
				var labelInput = E('input', { 'class': 'cbi-input-text', 'placeholder': _('名称') });
				var latInput = E('input', { 'class': 'cbi-input-text', 'placeholder': '51.5074' });
				var lonInput = E('input', { 'class': 'cbi-input-text', 'placeholder': '-0.1278' });
				ui.showModal(_('添加已保存位置'), [
					E('p', {}, [
						labelInput, ' ', latInput, ' ', lonInput
					]),
					E('div', { 'class': 'right' }, [
						E('button', { 'class': 'btn', 'click': ui.hideModal }, _('Cancel')),
						' ',
						E('button', { 'class': 'btn cbi-button-positive', 'click': function() {
							var label = labelInput.value.trim();
							var lat = latInput.value.trim();
							var lon = lonInput.value.trim();
							if (!label || !lat || !lon) return;
							var sid = uci.add('wloc-service', 'preset');
							uci.set('wloc-service', sid, 'label', label);
							uci.set('wloc-service', sid, 'latitude', lat);
							uci.set('wloc-service', sid, 'longitude', lon);
							uci.save('wloc-service').then(function() {
								return ui.changes.apply(true);
							}).then(function() {
								ui.hideModal();
								renderPresets();
								notify(_('已应用'), _('预设已保存。'));
							}).catch(function(e) {
								notify(_('应用失败'), String(e));
							});
						} }, _('Save'))
					])
				]);
			}
		}, _('添加已保存位置'));

		var presetsBox = E('div', { 'class': 'cbi-section' }, [
			E('h3', {}, _('已保存的位置')),
			presetTable,
			E('p', {}, addPresetBtn)
		]);

		/* ---------- 1. Safari certificate ---------- */
		var certLink = E('a', { 'href': PROFILE_URL, 'target': '_blank', 'id': 'wloc-cert-link' }, PROFILE_URL);
		var certText = E('div', { 'class': 'cbi-value' }, [
			E('label', { 'class': 'cbi-value-title' }, _('描述文件链接')),
			E('div', { 'class': 'cbi-value-field' }, certLink)
		]);

		var regenBtn = E('button', {
			'class': 'cbi-button cbi-button-apply',
			'id': 'wloc-regen-btn',
			'click': function() {
				this.disabled = true;
				regenProfile().then(function(r) {
					regenBtn.disabled = false;
					if (r.error) {
						notify(_('重新生成失败'), r.error);
						return;
					}
					notify(_('描述文件已就绪'),
						_('在 iPhone 上用 Safari 打开 %s，然后在 设置 > 通用 > 关于本机 > 证书信任设置 中开启完全信任。')
							.format(r.url || PROFILE_URL));
				});
			}
		}, _('重新生成描述文件'));

		var certBox = E('div', { 'class': 'cbi-section' }, [
			E('h3', {}, _('证书（Safari 安装）')),
			certText,
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' }, [
					E('label', { 'class': 'cbi-value-title' }, _('安装步骤')),
					E('div', { 'class': 'cbi-value-field' },
						_('1. 在测试 iPhone 的 Safari 中打开描述文件链接。 ') +
						_('2. 安装配置描述文件。 ') +
						_('3. 在 设置 > 通用 > 关于本机 > 证书信任设置 中开启完全信任。'))
				])),
			E('div', { 'class': 'cbi-row' },
				E('div', { 'class': 'cbi-value' }, [
					E('label', { 'class': 'cbi-value-title' }, ''),
					E('div', { 'class': 'cbi-value-field' }, regenBtn)
				]))
		]);

		return m.render().then(function(formNode) {
			return E([], [formNode, searchBox, presetsBox, certBox]);
		});
	}
});
