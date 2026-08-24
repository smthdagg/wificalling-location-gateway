#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
init="$repo_root/openwrt/files/etc/init.d/wloc-service"

grep -F 'WLOC_SYNTH_RESPONSE=1' "$init" >/dev/null || {
	echo 'FAIL: AX6S WLOC service does not enable the existing local synthesis path' >&2
	exit 1
}

if grep -F 'WLOC_DUMP_DIR=' "$init" >/dev/null; then
	echo 'FAIL: production WLOC service must not dump raw request/response bodies' >&2
	exit 1
fi

if grep -F 'wloc-forward.dump' "$repo_root/src/mitm/http1.rs" >/dev/null; then
	echo 'FAIL: HTTP/1.1 proxy must not dump raw request bodies' >&2
	exit 1
fi

if grep -F 'request from {client_addr}' "$repo_root/src/mitm/proxy.rs" >/dev/null; then
	echo 'FAIL: WLOC proxy logs must not expose the client IP address' >&2
	exit 1
fi

if grep -F 'wire_preview' "$repo_root/src/mitm/http1.rs" >/dev/null; then
	echo 'FAIL: HTTP/1.1 proxy logs must not expose forwarded request headers' >&2
	exit 1
fi

echo 'WLOC local synthesis default passed'
