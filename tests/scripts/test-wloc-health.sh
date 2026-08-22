#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
health="$repo_root/openwrt/files/usr/sbin/wloc-health.sh"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-health.XXXXXX")
trap 'if [ -n "${socket_pid:-}" ]; then kill "$socket_pid" 2>/dev/null || true; fi; rm -rf "$tmp"' EXIT HUP INT TERM

sh -n "$health"
mkdir -p "$tmp/bin"
cat > "$tmp/bin/pgrep" <<'EOF'
#!/bin/sh
printf '%s\n' 4242
EOF
cat > "$tmp/bin/uci" <<'EOF'
#!/bin/sh
exit 1
EOF
cat > "$tmp/bin/nft" <<'EOF'
#!/bin/sh
case "$*" in
  'list tables inet')
    printf '%s\n' 'table inet wloc_profile_phone' 'table inet wloc_profile_tablet'
    ;;
  'list table inet wloc_service')
    exit 1
    ;;
  'list table inet wloc_profile_phone'|'list table inet wloc_profile_tablet')
    printf '%s\n' 'table inet wloc_profile_phone {' ' chain prerouting { tproxy to :8443; meta mark set 1; }'
    ;;
  *)
    exit 1
    ;;
esac
EOF
cat > "$tmp/bin/provider-helper" <<EOF
#!/bin/sh
case "\$1" in
  path) printf '%s\n' "$tmp/provider" ;;
  *) exit 1 ;;
esac
EOF
cat > "$tmp/provider" <<'EOF'
#!/bin/sh
[ "${1:-}" = check ] && exit 0
exit 1
EOF
cat > "$tmp/profile-status" <<'EOF'
#!/bin/sh
printf '%s\n' '{"profiles":[]}'
EOF
chmod 0755 "$tmp/bin/pgrep" "$tmp/bin/uci" "$tmp/bin/nft" \
  "$tmp/bin/provider-helper" "$tmp/provider" "$tmp/profile-status"
printf '%s\n' '{"service_phase":"intercepting","exit":{"state":"manual"},"geo":{"state":"manual"},"last_error":null}' > "$tmp/status.json"
printf '{}\n' > "$tmp/sing-box.json"

python3 - "$tmp/control.sock" <<'PY' &
socket_path = __import__('sys').argv[1]
socket = __import__('socket')
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(socket_path)
server.listen(1)
__import__('signal').pause()
PY
socket_pid=$!
for _attempt in 1 2 3 4 5; do
  [ -S "$tmp/control.sock" ] && break
  sleep 1
done
[ -S "$tmp/control.sock" ]

report=$(PATH="$tmp/bin:$PATH" \
  WLOC_PROVIDER_HELPER="$tmp/bin/provider-helper" \
  WLOC_HEALTH_STATUS_FILE="$tmp/status.json" \
  WLOC_HEALTH_SOCKET="$tmp/control.sock" \
  WLOC_HEALTH_CONFIG_PATH="$tmp/sing-box.json" \
  WLOC_HEALTH_PROFILE_STATUS="$tmp/profile-status" \
  "$health")
printf '%s\n' "$report" | grep -F '"wloc":{"running":1,"socket":1,"status_fresh":1' >/dev/null
printf '%s\n' "$report" | grep -F '"provider":{"available":1,"valid":1,"config_present":1,"config_valid":1' >/dev/null
printf '%s\n' "$report" | grep -F '"redirect":{"table_present":1,"rules":1}' >/dev/null

printf '%s\n' 'wloc health shell tests passed'
