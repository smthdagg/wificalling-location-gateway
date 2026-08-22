#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
init="$repo_root/openwrt/files/etc/init.d/wloc-service"

grep -F 'WLOC_SYNTH_RESPONSE=1' "$init" >/dev/null || {
	echo 'FAIL: AX6S WLOC service does not enable the existing local synthesis path' >&2
	exit 1
}

echo 'WLOC local synthesis default passed'
