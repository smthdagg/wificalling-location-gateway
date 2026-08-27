'use strict';
'require baseclass';

function decodeLabel(value) {
	try { return decodeURIComponent(value || ''); } catch (e) { return value || ''; }
}

function decodeBase64(value) {
	var normalized = value.replace(/-/g, '+').replace(/_/g, '/').replace(/\s+/g, '');
	while (normalized.length % 4) normalized += '=';
	var binary = atob(normalized), bytes = new Uint8Array(binary.length);
	for (var i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	return new TextDecoder('utf-8').decode(bytes);
}

function truthy(value) {
	return /^(1|true|yes)$/i.test(value || '') ? '1' : '0';
}

function normalizeLink(value) {
	// Some subscription/export tools escape URI delimiters for Markdown or
	// config files (for example `vless\\://` and `pbk=abc\\_def`).
	return value.trim().replace(/\\(?=[:/?#&=_])/g, '');
}

function common(protocol, url) {
	if (!url.hostname || !url.port) throw new Error(_('Server and port are required'));
	return {
		enabled: '1', protocol: protocol, server: url.hostname, port: url.port,
		label: decodeLabel(url.hash.replace(/^#/, '')) || protocol.toUpperCase() + ' ' + url.hostname
	};
}

function userSecret(url) {
	return decodeURIComponent(url.password || url.username || '');
}

function parseUrl(uri, protocol) {
	var url = new URL(uri), p = url.searchParams, out = common(protocol, url);
	if (protocol === 'anytls' || protocol === 'hysteria2' || protocol === 'trojan') {
		out.password = userSecret(url);
		out.sni = p.get('peer') || p.get('sni') || '';
		out.insecure = truthy(p.get('insecure') || p.get('allowInsecure'));
		out.alpn = p.get('alpn') || '';
		// URLSearchParams decodes '+' to a space; the SHA-256 pin is
		// standard-alphabet base64 (every other link carries a '+'), so
		// restore it like the WireGuard private_key below.
		out.pin_sha256 = (p.get('pinSHA256') || '').replace(/ /g, '+');
		out.fingerprint = p.get('fingerprint') || p.get('fp') || '';
		out.udp = truthy(p.get('udp'));
	} else if (protocol === 'tuic') {
		out.uuid = decodeURIComponent(url.username || '');
		out.password = decodeURIComponent(url.password || '');
		out.sni = p.get('sni') || '';
		out.insecure = truthy(p.get('insecure') || p.get('allowInsecure') || p.get('allow_insecure'));
		out.alpn = p.get('alpn') || '';
		out.congestion = p.get('congestion_control') || p.get('congestion') || 'bbr';
		out.udp_mode = p.get('udp_relay_mode') || 'native';
	} else if (protocol === 'vless') {
		out.uuid = decodeURIComponent(url.username || '');
		out.flow = p.get('flow') || '';
		// pbk/sid are base64url in practice, but some generators emit
		// standard base64, which URLSearchParams would corrupt the same
		// way (see pinSHA256): restoring '+' is a no-op on base64url.
		out.public_key = (p.get('pbk') || p.get('publicKey') || '').replace(/ /g, '+');
		out.short_id = (p.get('sid') || p.get('shortId') || '').replace(/ /g, '+');
		out.security = p.get('security') || '';
		if (!out.security && out.public_key && out.short_id) out.security = 'reality';
		if (!out.security && truthy(p.get('tls')) === '1') out.security = 'tls';
		out.sni = p.get('sni') || p.get('peer') || '';
		if (!out.flow && p.get('xtls') === '2') out.flow = 'xtls-rprx-vision';
		out.fingerprint = p.get('fp') || p.get('fingerprint') || 'chrome';
		var vless_type = p.get('type') || '';
		if (vless_type === 'xhttp') {
			// XHTTP is a clash/mihomo transport; sing-box has no xhttp
			// transport, so the node would never connect here.
			throw new Error(_('xhttp transport is not supported by sing-box (use ws/grpc/httpupgrade)'));
		}
		if (vless_type === 'ws' || vless_type === 'grpc' || vless_type === 'httpupgrade') {
			out.transport = vless_type;
			out.host = p.get('host') || '';
			// grpc carries no path; its service_name goes in the path slot
			// (the UCI/normalized.conf layout has no separate field).
			out.path = (vless_type === 'grpc')
				? (p.get('serviceName') || p.get('service_name') || '/')
				: (p.get('path') || '/');
		}
	} else if (protocol === 'wireguard') {
		// wg://<peer_public_key>@<server>:<port>?private_key=…&local_address=…&reserved=…&mtu=…
		out.public_key = decodeURIComponent(url.username || '');
		// URLSearchParams decodes '+' to a space, which corrupts the
		// base64 private key; base64 never contains spaces, so restore.
		out.private_key = (p.get('private_key') || '').replace(/ /g, '+');
		out.local_address = (p.get('local_address') || p.get('ip') || '').split(',')[0] || '';
		out.reserved = p.get('reserved') || '';
		out.mtu = p.get('mtu') || '';
	}
	return out;
}

function parseVmess(uri) {
	var raw = JSON.parse(decodeBase64(uri.slice(uri.indexOf('://') + 3).trim()));
	if (!raw.add || !raw.port || !raw.id) throw new Error(_('VMess server, port and UUID are required'));
	var out = {
		enabled: '1', protocol: 'vmess', label: raw.ps || 'VMess ' + raw.add,
		server: raw.add, port: String(raw.port), uuid: raw.id, alter_id: String(raw.aid || 0),
		sni: raw.sni || '', host: raw.host || '', path: raw.path || '',
		security: raw.tls === 'tls' ? 'tls' : ''
	};
	var raw_net = raw.net || '';
	if (raw_net === 'xhttp') {
		throw new Error(_('xhttp transport is not supported by sing-box (use ws/grpc/httpupgrade)'));
	}
	if (raw_net === 'ws' || raw_net === 'grpc' || raw_net === 'httpupgrade') {
		out.transport = raw_net;
		out.host = raw.host || '';
		// grpc: service_name goes in the path slot.
		out.path = (raw_net === 'grpc') ? (raw.path || '/') : (raw.path || '/');
	}
	return out;
}

function parse(uri) {
	var value = normalizeLink(uri || ''), scheme = value.split(':', 1)[0].toLowerCase();
	if (scheme === 'vmess') return parseVmess('vmess://' + value.slice(value.indexOf('://') + 3));
	if (scheme === 'hy2') scheme = 'hysteria2';
	if (scheme === 'wg' || scheme === 'awg') scheme = 'wireguard';
	if (['anytls', 'hysteria2', 'tuic', 'vless', 'trojan', 'wireguard'].indexOf(scheme) < 0)
		throw new Error(_('Unsupported node link format'));
	if (scheme === 'vless') {
		var authorityStart = value.indexOf('://') + 3;
		var queryStart = value.indexOf('?', authorityStart);
		var authority = value.slice(authorityStart, queryStart < 0 ? value.length : queryStart);
		if (authority.indexOf('@') < 0) {
			try {
				var decodedAuthority = decodeBase64(authority);
				var legacyAuthority = decodedAuthority.match(/^(?:auto:|:)([^@]+)@(.+)$/);
				if (legacyAuthority) {
					value = 'vless://' + encodeURIComponent(legacyAuthority[1]) + '@' + legacyAuthority[2] +
						(queryStart < 0 ? '' : value.slice(queryStart));
				}
			} catch (e) {}
		}
	}
	return parseUrl(value, scheme);
}

return baseclass.extend({ parse: parse });
