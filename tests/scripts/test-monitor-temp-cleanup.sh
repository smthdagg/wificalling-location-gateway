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

printf '%s\n' 'Test device|192.0.2.10|node-test' > "$clients"
printf '%s\n' \
	'ipv4 2 udp 17 150 src=192.0.2.10 dst=198.51.100.10 sport=4500 dport=4500 packets=20 bytes=5000 src=198.51.100.10 dst=192.0.2.10 sport=4500 dport=4500 packets=18 bytes=4000 [ASSURED]' \
	'ipv4 2 udp 17 150 src=192.0.2.10 dst=198.51.100.11 sport=4501 dport=4500 packets=30 bytes=6000 src=198.51.100.11 dst=192.0.2.10 sport=4500 dport=4501 packets=28 bytes=5000 [ASSURED]' > "$conntrack"

"$repo_root/openwrt/files/usr/libexec/wificalling-gateway/monitor.sh" \
	"$clients" "$conntrack" "$status" "$state" "$events" 60 20 1

[ -s "$status" ] || {
	echo 'FAIL: monitor did not write status output' >&2
	exit 1
}

grep -F '"channel_count":2' "$status" >/dev/null || {
	echo 'FAIL: monitor collapsed two UDP 4500 channels into one' >&2
	exit 1
}
grep -F '"epdg_ips":["198.51.100.10","198.51.100.11"]' "$status" >/dev/null || {
	echo 'FAIL: monitor did not retain both ePDG endpoints' >&2
	exit 1
}
grep -F '"sent_packets":50,"reply_packets":46' "$status" >/dev/null || {
	echo 'FAIL: monitor did not aggregate both channel packet counters' >&2
	exit 1
}

leftovers=$(find "$work" -maxdepth 1 -type f -name 'events.log.tmp.*' | wc -l | tr -d ' ')
[ "$leftovers" -eq 0 ] || {
	echo "FAIL: monitor leaked $leftovers events.log temporary file(s)" >&2
	exit 1
}

echo 'monitor temporary-file cleanup passed'
