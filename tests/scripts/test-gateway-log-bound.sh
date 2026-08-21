#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
monitor="$repo_root/openwrt/files/usr/libexec/wificalling-gateway/monitor.sh"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-gateway-log.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

sh -n "$monitor"
printf '%s\n' 'phone|192.168.1.10|node-phone' > "$tmp/clients"
: > "$tmp/conntrack"
: > "$tmp/status.json"
: > "$tmp/state"
i=0
while [ "$i" -lt 40 ]; do
  printf '%s|phone|192.168.1.10|handshake_failed|0|0|call_or_sms_unknown|not_detected\n' "$i" >> "$tmp/events.log"
  i=$((i + 1))
done

WFC_NOW=100 WFC_MAX_EVENT_LOG_BYTES=512 "$monitor" \
  "$tmp/clients" "$tmp/conntrack" "$tmp/status.json" "$tmp/state" "$tmp/events.log" 60 20 1

bytes=$(wc -c < "$tmp/events.log" | tr -d ' ')
[ "$bytes" -le 512 ] || {
  printf 'event log exceeds byte bound: %s\n' "$bytes" >&2
  exit 1
}
awk -F '|' 'NF != 8 { exit 1 }' "$tmp/events.log"

printf '%s\n' 'gateway log bound tests passed'
