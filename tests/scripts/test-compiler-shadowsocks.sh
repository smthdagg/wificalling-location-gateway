#!/bin/sh
set -eu

# Shadowsocks reuses the generic credential/auxiliary slots (f[6]/f[10]) rather
# than widening the | -delimited record, so the emitted outbound is the only
# place that proves the mapping is still right.  Credentials here are synthetic.

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/wfc-compiler-ss.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM

compiler="$repo_root/openwrt/files/usr/libexec/wificalling-gateway/compiler.sh"
normalized="$work/normalized.conf"
output="$work/sing-box.json"

# node record: node|id|proto|server|port|credential|sni|insecure|alpn|auxiliary|<15 empty>
write_node() {
	method=$1
	{
		printf 'global|log_level|warn\n'
		printf 'global|wireguard_style|legacy\n'
		printf 'probe|ssnode|20001\n'
		printf 'node|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s\n' \
			ssnode shadowsocks example.test 8388 s3cret '' 0 '' "$method" \
			'' '' '' '' '' '' '' '' '' '' '' '' '' '' ''
		printf 'device|testdev|ssnode|192.168.1.100\n'
	} > "$normalized"
}

# 1. A complete node compiles into a shadowsocks outbound.
write_node aes-256-gcm
sh "$compiler" "$normalized" "$output"

grep -q '"type":"shadowsocks","tag":"node-ssnode"' "$output" || {
	echo 'FAIL: shadowsocks outbound missing or mistagged' >&2
	exit 1
}
grep -q '"method":"aes-256-gcm"' "$output" || {
	echo 'FAIL: cipher not taken from the auxiliary slot' >&2
	exit 1
}
grep -q '"password":"s3cret"' "$output" || {
	echo 'FAIL: password not taken from the credential slot' >&2
	exit 1
}
# Shadowsocks has no TLS layer: emitting one would be rejected by sing-box.
if grep -q '"tag":"node-ssnode","server":"example.test","server_port":8388,"method":"aes-256-gcm","password":"s3cret","tls"' "$output"; then
	echo 'FAIL: shadowsocks outbound must not carry a tls block' >&2
	exit 1
fi
# The device policy must still route to the node.
grep -q '"source_ip_cidr":\["192.168.1.100/32"\],"action":"route","outbound":"node-ssnode"' "$output" || {
	echo 'FAIL: device policy does not route to the shadowsocks node' >&2
	exit 1
}

# 2. An unsupported cipher must fail the whole compile with an explicit
#    message, not emit an outbound that would make sing-box reject the entire
#    config and take every other node down with it.
if write_node 'rc4-md5' && sh "$compiler" "$normalized" "$output" 2>/dev/null; then
	echo 'FAIL: unsupported cipher must abort the compile' >&2
	exit 1
fi
write_node 'rc4-md5'
sh "$compiler" "$normalized" "$output" 2>&1 | grep -F 'uses an unsupported encryption method' >/dev/null || {
	echo 'FAIL: unsupported cipher must fail with an explicit message' >&2
	exit 1
}

# 3. An empty cipher must fail the whole compile with an explicit message,
#    not emit a broken outbound that would make sing-box reject every other
#    node with it.
write_node ''
if sh "$compiler" "$normalized" "$work/rejected.json" 2>"$work/err"; then
	echo 'FAIL: compiler accepted a shadowsocks node without a cipher' >&2
	exit 1
fi
grep -q 'missing the encryption method' "$work/err" || {
	echo 'FAIL: missing-cipher rejection did not explain itself' >&2
	cat "$work/err" >&2
	exit 1
}
[ ! -f "$work/rejected.json" ] || {
	echo 'FAIL: compiler left a partial config behind after rejecting a node' >&2
	exit 1
}

echo 'compiler shadowsocks mapping passed'
