#!/bin/sh
set -eu
repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-log-bound.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
log="$tmp/events.jsonl"
out="$tmp/support.tar.gz"
i=0
while [ "$i" -lt 100 ]; do
  printf '{"timestamp":%s,"component":"wloc","severity":"info","event_code":"target_updated"}
' "$i" >> "$log"
  i=$((i + 1))
done
WLOC_SUPPORT_WLOC_LOG="$log" WLOC_SUPPORT_OUTPUT="$out" WLOC_SUPPORT_MAX_BYTES=4096   sh "$repo_root/openwrt/files/usr/sbin/wloc-support-bundle.sh" >/dev/null
[ -s "$out" ]
bytes=$(wc -c < "$out" | tr -d ' ')
[ "$bytes" -le 4096 ]
echo 'WLOC log bound tests passed'

