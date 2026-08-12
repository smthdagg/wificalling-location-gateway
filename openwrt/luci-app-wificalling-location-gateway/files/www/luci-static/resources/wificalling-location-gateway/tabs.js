'use strict';
'require baseclass';

// Tab 标题本地化：菜单 title 使用英文原文（英文界面直接显示），中文
// 界面在这里替换为中文。与 LuCI 自身的 _() 翻译机制无关——本插件没有
// 可编译的 .lmo 翻译文件。

var TAB_LABELS = {
	'Wi-Fi Calling Settings': 'Wi-Fi 通话设置',
	'Wi-Fi Calling Monitor & Log': 'Wi-Fi 通话监控与日志',
	'WLOC Settings': 'WLOC 设置',
	'WLOC Monitor & Log': 'WLOC 监控与日志',
	'Help (FAQ)': '使用帮助（FAQ）'
};

return baseclass.extend({
	// Replace the top tab labels with the Chinese names when the LuCI
	// interface language is Chinese; English stays as the menu titles.
	localize: function() {
		var cls = document.body.className;
		if (cls.indexOf('lang-en') >= 0 || cls.indexOf('lang_en') >= 0)
			return;
		document.querySelectorAll('#tabmenu a, .tabs a').forEach(function(a) {
			var text = a.textContent.trim();
			if (TAB_LABELS[text])
				a.textContent = TAB_LABELS[text];
		});
	}
});
