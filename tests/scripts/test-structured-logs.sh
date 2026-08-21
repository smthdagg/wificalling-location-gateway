#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-structured-log.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

events="$tmp/events.jsonl"
output="$tmp/support.tar.gz"
cat > "$events" <<'EOF'
{"timestamp":1000,"component":"wloc","profile_scope":"device-policy","severity":"info","event_code":"target_updated","fields":{"assigned_device":"192.168.1.100","label":"Phone","latitude":1.2}}
{"timestamp":1001,"component":"wloc","profile_scope":"device-policy","severity":"warn","event_code":"probe_failed","fields":{"node_ref":"private-node"}}
EOF
WLOC_SUPPORT_WLOC_LOG="$events" WLOC_SUPPORT_OUTPUT="$output"   sh "$repo_root/openwrt/files/usr/sbin/wloc-support-bundle.sh" >/dev/null
[ -s "$output" ]
tar -xOzf "$output" ./wloc-support/events.jsonl > "$tmp/redacted"
grep '"component":"wloc"' "$tmp/redacted" >/dev/null
grep '"message":"redacted diagnostic event"' "$tmp/redacted" >/dev/null
if grep -E '192\.168\.1\.100|Phone|1\.2|private-node' "$tmp/redacted" >/dev/null; then
  echo "redacted event leaked device material" >&2
  exit 1
fi
bytes=$(wc -c < "$output" | tr -d ' ')
[ "$bytes" -le 65536 ]
echo 'structured logs tests passed'
