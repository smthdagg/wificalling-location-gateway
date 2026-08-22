#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/wfc-monitor-cleanup.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM

clients="$work/clients"
conntrack="$work/nf_conntrack"
status="$work/status.json"
state="$work/monitor.state"
events="$work/events.log"
monitor="$repo_root/openwrt/files/usr/libexec/wificalling-gateway/monitor.sh"

if [ ! -f "$monitor" ]; then
	echo 'monitor source is supplied by the bundled Gateway IPK; package-level cleanup test required'
	exit 0
fi

printf '%s\n' 'Test device|192.0.2.10|node-test' > "$clients"
: > "$conntrack"

"$monitor" \
	"$clients" "$conntrack" "$status" "$state" "$events" 60 20 1

[ -s "$status" ] || {
	echo 'FAIL: monitor did not write status output' >&2
	exit 1
}

leftovers=$(find "$work" -maxdepth 1 -type f -name 'events.log.tmp.*' | wc -l | tr -d ' ')
[ "$leftovers" -eq 0 ] || {
	echo "FAIL: monitor leaked $leftovers events.log temporary file(s)" >&2
	exit 1
}

echo 'monitor temporary-file cleanup passed'
