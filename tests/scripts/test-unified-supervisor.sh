#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
supervisor="$repo_root/openwrt/files/usr/libexec/wificalling-location-gateway/unified-supervisor.sh"
init="$repo_root/openwrt/files/etc/init.d/wificalling-location-gateway"
redirect="$repo_root/openwrt/files/usr/sbin/wloc-redirect-sync.sh"
wloc_refresh="$repo_root/openwrt/files/usr/sbin/wloc-refresh-set.sh"
wloc_init="$repo_root/openwrt/files/etc/init.d/wloc-service"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-unified-supervisor.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

sh -n "$supervisor" "$init" "$redirect"
sh -n "$wloc_refresh"
sh -n "$wloc_init"
grep -F 'procd_set_param command "$SUPERVISOR" start' "$init" >/dev/null
grep -F 'procd_set_param respawn 3600 5 3' "$init" >/dev/null
grep -F 'procd_add_reload_trigger wificalling-gateway' "$init" >/dev/null
grep -F 'procd_add_reload_trigger wloc-service' "$init" >/dev/null
grep -F 'WLOC_SUPERVISED=1 WLOC_DEFER_REDIRECT=1 WLOC_SKIP_REDIRECT=1 "$WLOC_INIT" start' "$supervisor" >/dev/null
grep -F 'WLOC_SUPERVISED=1 "$GATEWAY_INIT" start' "$supervisor" >/dev/null
grep -F 'WLOC_DEFER_REDIRECT=1' "$supervisor" >/dev/null
grep -F 'PROFILE_REDIRECT_HELPER' "$supervisor" >/dev/null
grep -F 'stop-all' "$supervisor" >/dev/null
grep -F 'legacy-stop' "$supervisor" "$redirect" >/dev/null
grep -F 'multiple_profiles_configured' "$redirect" >/dev/null
grep -F 'PROFILE_PROXY_READY_FILE' "$supervisor" >/dev/null
grep -F 'PROFILE_ACTIVATE_FILE' "$supervisor" >/dev/null
grep -F 'PROFILE_READY_FILE' "$supervisor" >/dev/null
grep -F 'REFRESH_SET_HELPER' "$supervisor" >/dev/null
grep -F 'refresh-set' "$supervisor" >/dev/null

if grep -E 'pgrep[[:space:]]+-f' "$supervisor" >/dev/null; then
	printf '%s\n' 'supervisor must not use global pgrep -f process matching' >&2
	exit 1
fi
grep -F 'WLOC_SERVICE_PIDFILE' "$supervisor" >/dev/null
grep -F 'service_pid_matches' "$supervisor" >/dev/null
grep -F 'health)' "$supervisor" >/dev/null

mkdir -p "$tmp/health-bin" "$tmp/gateway"
cat > "$tmp/health-bin/nft" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod 0755 "$tmp/health-bin/nft"
: > "$tmp/gateway/sing-box.json"
sleep 60 & stale_pid=$!
printf '%s\n' "$stale_pid" > "$tmp/stale-wloc.pid"
if WLOC_UNIFIED_RUNDIR="$tmp/run" \
	WLOC_SERVICE_PIDFILE="$tmp/stale-wloc.pid" \
	WLOC_SERVICE_BIN=/usr/sbin/wloc-service \
	WLOC_SOCKET="$tmp/control.sock" \
	GATEWAY_CONFIG="$tmp/gateway/sing-box.json" \
	GATEWAY_NFT_BINARY="$tmp/health-bin/nft" \
		"$supervisor" health; then
	health_rc=0
else
	health_rc=$?
fi
kill "$stale_pid" 2>/dev/null || true
wait "$stale_pid" 2>/dev/null || true
[ "$health_rc" -eq 1 ]

grep -F 'WLOC_PROFILE_ACTIVATE_FILE' "$repo_root/src/bin/wloc-service.rs" >/dev/null
grep -F 'service.activate_profiles()' "$repo_root/src/bin/wloc-service.rs" >/dev/null
grep -F 'WLOC_SKIP_REDIRECT' "$wloc_init" >/dev/null
grep -F 'WLOC_SUPERVISED' "$repo_root/openwrt/files/etc/init.d/wificalling-gateway" >/dev/null

gateway_start=$(grep -n '"$GATEWAY_INIT" start' "$supervisor" | cut -d: -f1)
wloc_start=$(grep -n '"$WLOC_INIT" start' "$supervisor" | cut -d: -f1)
health=$(grep -n 'if ! wait_for_health' "$supervisor" | head -n 1 | cut -d: -f1)
redirect_start=$(grep -n '^[[:space:]]*if ! install_redirect' "$supervisor" | cut -d: -f1)
[ "$gateway_start" -lt "$wloc_start" ]
[ "$wloc_start" -lt "$health" ]
[ "$health" -lt "$redirect_start" ]
grep -F 'cleanup_runtime wloc_start_failed 1' "$supervisor" >/dev/null
grep -F 'cleanup_runtime health_failed 1' "$supervisor" >/dev/null
if grep -F 'stop_child "$GATEWAY_INIT"' "$supervisor" >/dev/null; then
	printf '%s\n' 'unified supervisor must not stop the stable Gateway table owner' >&2
	exit 1
fi
grep -F 'START_TIMEOUT' "$supervisor" >/dev/null

if grep -E 'udp[[:space:]]+500|udp[[:space:]]+4500|wificalling_gateway' "$supervisor" "$redirect" >/dev/null; then
	printf '%s\n' 'unified supervisor must not own Gateway nftables or UDP 500/4500' >&2
	exit 1
fi

mkdir -p "$tmp/bin"
cat > "$tmp/bin/uci" <<'EOF'
#!/bin/sh
if [ "$2" = show ]; then
  printf '%s\n' 'wloc-service.phone=device' 'wloc-service.tablet=device'
fi
EOF
chmod 0755 "$tmp/bin/uci"
cat > "$tmp/bin/nft" <<'EOF'
#!/bin/sh
printf 'nft %s\n' "$*" >> "$WLOC_TEST_LOG"
EOF
cat > "$tmp/bin/ip" <<'EOF'
#!/bin/sh
printf 'ip %s\n' "$*" >> "$WLOC_TEST_LOG"
EOF
chmod 0755 "$tmp/bin/nft" "$tmp/bin/ip"
: > "$tmp/commands.log"
if ! PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" \
  WLOC_NFT_BINARY="$tmp/bin/nft" WLOC_IP_BINARY="$tmp/bin/ip" \
  WLOC_PROFILE_REDIRECT_HELPER="$repo_root/openwrt/files/usr/sbin/wloc-profile-redirect.sh" \
  "$redirect" start; then

  printf '%s\n' 'profile mode must preserve the shared TPROXY route' >&2
  exit 1
fi
grep -F 'nft delete table inet wloc_service' "$tmp/commands.log" >/dev/null
grep -F 'ip rule add fwmark 1 lookup 100' "$tmp/commands.log" >/dev/null
grep -F 'ip route add local 0.0.0.0/0 dev lo table 100' "$tmp/commands.log" >/dev/null
if grep -F 'nft add table inet wloc_service' "$tmp/commands.log" >/dev/null; then
  printf '%s\n' 'multi-profile mode must not install the legacy all-device table' >&2
  exit 1
fi
if grep -E 'gs-loc-corpa|apple\.com\.cn|bluedot' "$redirect" "$wloc_refresh" >/dev/null; then
	printf '%s\n' 'WLOC scope must remain limited to the two exact Apple hostnames' >&2
	exit 1
fi

mkdir -p "$tmp/bin" "$tmp/hosts"
printf '%s\n' '# wloc-service DNS hijack (do not edit)' '192.0.2.1 old.example' '# wloc-service end' > "$tmp/hosts/a"
printf '%s\n' '# keep' > "$tmp/hosts/b"
cat > "$tmp/bin/nft" <<'EOF'
#!/bin/sh
printf 'nft %s\n' "$*" >> "$WLOC_TEST_LOG"
EOF
cat > "$tmp/bin/ip" <<'EOF'
#!/bin/sh
printf 'ip %s\n' "$*" >> "$WLOC_TEST_LOG"
EOF
chmod 0755 "$tmp/bin/nft" "$tmp/bin/ip"
: > "$tmp/commands.log"
PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" \
	WLOC_HOSTS_FILES="$tmp/hosts/a $tmp/hosts/b" "$redirect" stop
grep -F 'nft delete table inet wloc_service' "$tmp/commands.log" >/dev/null
grep -F 'ip rule del fwmark 1 lookup 100' "$tmp/commands.log" >/dev/null
grep -F '# keep' "$tmp/hosts/b" >/dev/null
if grep -F 'wificalling-gateway' "$tmp/commands.log" >/dev/null; then
	printf '%s\n' 'WLOC cleanup touched the Gateway namespace' >&2
	exit 1
fi

printf '%s\n' 'unified supervisor shell tests passed'
