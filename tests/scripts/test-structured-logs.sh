#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-structured-log.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

clients="$tmp/clients"
conntrack="$tmp/conntrack"
status="$tmp/status.json"
events="$tmp/events.log"
state="$tmp/state"
printf 'Phone|192.168.1.100|node-a\n' > "$clients"
: > "$conntrack"

WFC_NOW=1000 WFC_MAX_EVENT_LOG_BYTES=2048 \
  sh "$repo_root/openwrt/files/usr/libexec/wificalling-gateway/monitor.sh" \
  "$clients" "$conntrack" "$status" "$state" "$events" 30 20 1

if [ -s "$events" ]; then
  echo "unexpected event without a transition" >&2
  exit 1
fi

cat > "$conntrack" <<'EOF'
ipv4 src=192.168.1.100 dst=10.0.0.2 sport=4500 dport=4500 packets=100 packets=100 [ASSURED]
EOF
WFC_NOW=1016 WFC_MAX_EVENT_LOG_BYTES=2048 \
  sh "$repo_root/openwrt/files/usr/libexec/wificalling-gateway/monitor.sh" \
  "$clients" "$conntrack" "$status" "$state" "$events" 30 20 1

line=$(tail -n 1 "$events")
printf '%s\n' "$line" | grep '"component":"gateway"' >/dev/null
printf '%s\n' "$line" | grep '"profile_scope":"device-policy"' >/dev/null
printf '%s\n' "$line" | grep '"severity":"info"' >/dev/null
printf '%s\n' "$line" | grep '"event_code":"handshake_success"' >/dev/null
if printf '%s\n' "$line" | grep -E '192\.168\.1\.100|Phone|"ip"|"label"' >/dev/null; then
  echo "structured event leaked device material" >&2
  exit 1
fi

bytes=$(wc -c < "$events" | tr -d ' ')
[ "$bytes" -le 2048 ]
echo 'structured logs tests passed'
