#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-support-test.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

cat > "$tmp/health.json" <<'EOF'
{"services":{"wloc":{"running":true},"gateway":{"running":true}},"profiles":[{"id":"phone","label":"Alice phone","assigned_device":"192.168.1.100"}],"geo":{"latitude":1.2,"longitude":3.4}}
EOF
cat > "$tmp/wloc.jsonl" <<'EOF'
{"timestamp":1000,"component":"wloc","profile_scope":"service","severity":"info","event_code":"target_updated","message":"secret 192.168.1.100","latitude":1.2,"longitude":3.4}
EOF
cat > "$tmp/gateway.jsonl" <<'EOF'
{"timestamp":1001,"component":"gateway","profile_scope":"device-policy","severity":"info","event_code":"handshake_success","message":"Alice phone 192.168.1.100","device":"Alice phone"}
EOF
mkdir -p "$tmp/profiles/phone"
cat > "$tmp/profiles/phone/events.jsonl" <<'EOF'
{"timestamp":1002,"component":"wloc","profile_scope":"phone","severity":"warning","event_code":"profile_degraded","message":"Alice phone 192.168.1.100","latitude":1.2,"longitude":3.4}
EOF

output="$tmp/support.tar.gz"
WLOC_SUPPORT_OUTPUT="$output" \
WLOC_SUPPORT_HEALTH="$tmp/health.json" \
WLOC_SUPPORT_WLOC_LOG="$tmp/wloc.jsonl" \
WLOC_SUPPORT_GATEWAY_LOG="$tmp/gateway.jsonl" \
WLOC_SUPPORT_PROFILE_LOG_ROOT="$tmp/profiles" \
WLOC_SUPPORT_MAX_BYTES=32768 \
  sh "$repo_root/openwrt/files/usr/sbin/wloc-support-bundle.sh"

[ -s "$output" ]
bytes=$(wc -c < "$output" | tr -d ' ')
[ "$bytes" -le 32768 ]
tar -tzf "$output" | grep '^wloc-support/' >/dev/null
tar -xOzf "$output" wloc-support/events.jsonl | grep '"event_code":"target_updated"' >/dev/null
tar -xOzf "$output" wloc-support/events.jsonl > "$tmp/redacted-events.jsonl"
grep '"event_code":"profile_degraded"' "$tmp/redacted-events.jsonl" >/dev/null
if grep -E 'Alice|192\.168\.1\.100|latitude|longitude|secret|"profile_scope":"phone"' "$tmp/redacted-events.jsonl" >/dev/null; then
  echo 'support bundle leaked private event material' >&2
  exit 1
fi
if tar -xOzf "$output" wloc-support/health.json | grep -E 'Alice|192\.168\.1\.100|profiles|latitude|longitude' >/dev/null; then
  echo 'support bundle leaked private health material' >&2
  exit 1
fi

victim="$tmp/victim"
printf 'do not replace this file\n' > "$victim"
ln -s "$victim" "$tmp/symlink.tar.gz"
if WLOC_SUPPORT_OUTPUT="$tmp/symlink.tar.gz" \
   WLOC_SUPPORT_HEALTH="$tmp/health.json" \
   WLOC_SUPPORT_WLOC_LOG="$tmp/wloc.jsonl" \
   WLOC_SUPPORT_GATEWAY_LOG="$tmp/gateway.jsonl" \
   sh "$repo_root/openwrt/files/usr/sbin/wloc-support-bundle.sh"; then
  echo 'support bundle accepted a symlink output' >&2
  exit 1
fi
grep 'do not replace this file' "$victim" >/dev/null
echo 'support bundle tests passed'
