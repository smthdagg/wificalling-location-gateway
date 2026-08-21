#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
supervisor="$repo_root/openwrt/files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh"
init="$repo_root/openwrt/files/etc/init.d/wificalling-location-gateway"
redirect="$repo_root/openwrt/files/usr/sbin/wloc-redirect-sync.sh"
wloc_init="$repo_root/openwrt/files/etc/init.d/wloc-service"

sh -n "$supervisor" "$init" "$redirect" "$wloc_init"
grep -F 'procd_set_param command "$SUPERVISOR" start' "$init" >/dev/null
grep -F 'procd_set_param respawn 3600 5 3' "$init" >/dev/null
grep -F 'procd_add_reload_trigger wloc-service' "$init" >/dev/null
grep -F 'WLOC_SERVICE_PIDFILE' "$supervisor" >/dev/null
grep -F 'service_pid_matches' "$supervisor" >/dev/null
grep -F 'provider_health' "$supervisor" >/dev/null
grep -F 'singbox_config' "$supervisor" >/dev/null
grep -F 'provider_available=1' "$supervisor" >/dev/null
grep -F 'multi_profile_mode' "$supervisor" >/dev/null
grep -F '[ "${profiles:-0}" -gt 1 ]' "$supervisor" >/dev/null
grep -F 'PROFILE_REDIRECT_HELPER' "$supervisor" >/dev/null
grep -F 'health)' "$supervisor" >/dev/null
grep -F 'wloc-service' "$supervisor" >/dev/null
grep -F 'singbox-runtime.sh' "$supervisor" >/dev/null
grep -F 'multiple_profiles_configured' "$redirect" >/dev/null
grep -F 'flags timeout' "$redirect" "$repo_root/openwrt/files/usr/sbin/wloc-profile-redirect.sh" >/dev/null
grep -F 'timeout 30s' "$redirect" "$repo_root/openwrt/files/usr/sbin/wloc-profile-redirect.sh" >/dev/null
if grep -E 'udp[[:space:]]+500|udp[[:space:]]+4500|nft[[:space:]]+(add|delete|flush|insert|replace).*wificalling_gateway' "$supervisor" "$redirect" >/dev/null; then
  echo 'WLOC must not own Gateway nftables or UDP 500/4500' >&2
  exit 1
fi
if grep -E 'GATEWAY_|wificalling-gateway' "$supervisor" "$redirect" "$init" "$wloc_init" >/dev/null; then
  echo 'standalone supervisor contains a Gateway dependency' >&2
  exit 1
fi
echo 'standalone supervisor shell tests passed'
