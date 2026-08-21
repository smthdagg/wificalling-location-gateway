#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
status="$repo_root/openwrt/files/usr/sbin/wloc-profile-status.sh"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-profile-status.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

sh -n "$status"
mkdir -p "$tmp/bin"
cat > "$tmp/bin/uci" <<'EOF'
#!/bin/sh
case "$*" in
  '-q show wloc-service')
    printf '%s\n' 'wloc-service.default=device' 'wloc-service.phone=device' 'wloc-service.tablet=device'
    ;;
  '-q get wloc-service.default.label') printf '%s\n' 'Default device' ;;
  '-q get wloc-service.default.enabled') printf '%s\n' '1' ;;
  '-q get wloc-service.default.assigned_device') printf '%s\n' '192.168.1.9' ;;
  '-q get wloc-service.default.node_ref') printf '%s\n' 'node-default' ;;
  '-q get wloc-service.phone.label') printf '%s\n' 'Living room phone' ;;
  '-q get wloc-service.phone.enabled') printf '%s\n' '1' ;;
  '-q get wloc-service.phone.assigned_device') printf '%s\n' '192.168.1.10' ;;
  '-q get wloc-service.phone.node_ref') printf '%s\n' 'node-phone' ;;
  '-q get wloc-service.tablet.label') printf '%s\n' 'Tablet' ;;
  '-q get wloc-service.tablet.enabled') printf '%s\n' '1' ;;
  '-q get wloc-service.tablet.assigned_device') printf '%s\n' '' ;;
  '-q get wloc-service.tablet.node_ref') printf '%s\n' 'node-tablet' ;;
  *) exit 1 ;;
esac
EOF
cat > "$tmp/bin/nft" <<'EOF'
#!/bin/sh
case "$*" in
  'list table inet wloc_profile_default') exit 0 ;;
  'list table inet wloc_profile_phone') exit 0 ;;
  *) exit 1 ;;
esac
EOF
chmod 0755 "$tmp/bin/uci" "$tmp/bin/nft"

output=$(PATH="$tmp/bin:$PATH" "$status")
printf '%s\n' "$output" | grep -F '"id":"default"' >/dev/null
printf '%s\n' "$output" | grep -F '"id":"default"' | grep -F '"phase":"intercepting"' >/dev/null
printf '%s\n' "$output" | grep -F '"id":"phone"' >/dev/null
printf '%s\n' "$output" | grep -F '"phase":"intercepting"' >/dev/null
printf '%s\n' "$output" | grep -F '"id":"tablet"' >/dev/null
printf '%s\n' "$output" | grep -F '"reason_code":"missing_device_binding"' >/dev/null
if printf '%s\n' "$output" | grep -E '192\.168\.1\.10|node-phone|node-tablet' >/dev/null; then
  printf '%s\n' 'profile status must not expose device addresses or node references' >&2
  exit 1
fi

printf '%s\n' 'profile status shell tests passed'
