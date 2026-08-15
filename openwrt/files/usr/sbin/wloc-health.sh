#!/bin/sh
# Service health report for the WLOC monitor page.
#
# Emits a single JSON document covering both services (wloc-service and
# the Wi-Fi Calling Gateway incl. sing-box), the node health document, the
# build patches, and the recent log lines. The LuCI "Service status" page
# renders it via the luci.wloc rpcd `health` method.
#
# Every check is defensive: a missing file or binary reports warn/error
# instead of failing the whole report.

set -eu

json_escape() {
	printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\t/\\t/g'
}

now=$(date +%s)

# --- wloc-service ---------------------------------------------------------
wloc_pid=$(pgrep -f '/usr/sbin/wloc-service' 2>/dev/null | head -n 1 || true)
wloc_running=0; [ -n "$wloc_pid" ] && wloc_running=1
wloc_socket=0; [ -S /var/run/wloc-service/control.sock ] && wloc_socket=1

wloc_phase=unknown; wloc_exit=unknown; wloc_geo=unknown; wloc_error=null; wloc_status_fresh=0
wloc_status=/var/run/wloc-service/status.json
if [ -f "$wloc_status" ]; then
	age=$((now - $(stat -c %Y "$wloc_status" 2>/dev/null || echo 0)))
	[ "$age" -le 120 ] && wloc_status_fresh=1
	wloc_phase=$(sed -n 's/.*"service_phase"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$wloc_status" | head -n 1)
	[ -n "$wloc_phase" ] || wloc_phase=unknown
	wloc_exit=$(sed -n 's/.*"exit"[[:space:]]*:[[:space:]]*{[^}]*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$wloc_status" | head -n 1)
	[ -n "$wloc_exit" ] || wloc_exit=unknown
	wloc_geo=$(sed -n 's/.*"geo"[[:space:]]*:[[:space:]]*{[^}]*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$wloc_status" | head -n 1)
	[ -n "$wloc_geo" ] || wloc_geo=unknown
	wloc_err=$(sed -n 's/.*"last_error"[[:space:]]*:[[:space:]]*\([^,}]*\).*/\1/p' "$wloc_status" | head -n 1)
	[ -n "$wloc_err" ] && wloc_error=$(json_escape "$wloc_err")
fi

# --- wificalling-gateway / sing-box ---------------------------------------
monitor_pid=$(pgrep -f 'monitor-loop.sh' 2>/dev/null | head -n 1 || true)
monitor_running=0; [ -n "$monitor_pid" ] && monitor_running=1
sb_pid=$(pgrep -f '/usr/bin/sing-box run' 2>/dev/null | head -n 1 || true)
sb_running=0; [ -n "$sb_pid" ] && sb_running=1

rundir=/var/run/wificalling-gateway
sb_config=0; sb_config_valid=0; sb_config_age=-1
if [ -f "$rundir/sing-box.json" ]; then
	sb_config=1
	sb_config_age=$((now - $(stat -c %Y "$rundir/sing-box.json" 2>/dev/null || echo 0)))
	if command -v sing-box >/dev/null 2>&1; then
		if sing-box check -c "$rundir/sing-box.json" >/dev/null 2>&1; then
			sb_config_valid=1
		fi
	fi
fi

norm_fresh=0; norm_age=-1
if [ -f "$rundir/normalized.conf" ]; then
	norm_age=$((now - $(stat -c %Y "$rundir/normalized.conf" 2>/dev/null || echo 0)))
	[ "$norm_age" -le 120 ] && norm_fresh=1
fi

nft_rules=0
if command -v nft >/dev/null 2>&1; then
	nft_rules=$(nft list ruleset 2>/dev/null | grep -c -E 'tproxy|redirect' || true)
fi

devices=$(grep -c '^device|' "$rundir/normalized.conf" 2>/dev/null || true)
[ -n "$devices" ] || devices=0

# --- build patches --------------------------------------------------------
patch_psk=0; patch_health=0; patch_compact=0; patch_device_guard=0
[ -f /usr/libexec/wificalling-gateway/compiler.sh ] && {
	grep -q 'pre_shared_key' /usr/libexec/wificalling-gateway/compiler.sh && patch_psk=1
	grep -q 'device_guard_marker' /usr/libexec/wificalling-gateway/compiler.sh && patch_device_guard=1
}
[ -f /usr/libexec/wificalling-gateway/node-health.sh ] && {
	grep -q 'wg_handshake_test' /usr/libexec/wificalling-gateway/node-health.sh && patch_health=1
	grep -q 'compact_status_marker' /usr/libexec/wificalling-gateway/node-health.sh && patch_compact=1
}

# --- node health ----------------------------------------------------------
nodes_total=0; nodes_ok=0; nodes_down=0; nodes_unknown=0
node_status=/www/wloc-node-status.json
if [ -f "$node_status" ]; then
	nodes_total=$(grep -o '"id":"' "$node_status" | wc -l)
	nodes_ok=$(grep -o '"state":"\(reachable\|tcp_reachable\|handshake_ok\)"' "$node_status" | wc -l)
	nodes_down=$(grep -o '"state":"\(unreachable\|handshake_failed\)"' "$node_status" | wc -l)
	nodes_unknown=$((nodes_total - nodes_ok - nodes_down))
	[ "$nodes_unknown" -lt 0 ] && nodes_unknown=0
fi

# --- log tail -------------------------------------------------------------
log_tail=$(logread 2>/dev/null | grep -E 'wloc-service|wificalling-gateway|sing-box' | tail -n 12 || true)
log_json='[]'
if [ -n "$log_tail" ]; then
	log_json=$(printf '%s\n' "$log_tail" | sed 's/\\/\\\\/g; s/"/\\"/g' | awk '
		BEGIN { printf "[" }
		{ if (NR > 1) printf ","; printf "\"%s\"", $0 }
		END { printf "]" }')
fi

printf '{"generated_at":%s,' "$now"
printf '"services":{"wloc":{"running":%s,"socket":%s,"status_fresh":%s,"phase":"%s","exit":"%s","geo":"%s","last_error":%s},' \
	"$wloc_running" "$wloc_socket" "$wloc_status_fresh" "$wloc_phase" "$wloc_exit" "$wloc_geo" "$wloc_error"
printf '"gateway":{"running":%s,"monitor":%s,"singbox":%s,"config_present":%s,"config_valid":%s,"config_age":%s,"normalized_fresh":%s,"nft_rules":%s,"devices":%s,' \
	"$monitor_running" "$monitor_running" "$sb_running" "$sb_config" "$sb_config_valid" "$sb_config_age" "$norm_fresh" "$nft_rules" "$devices"
printf '"patches":{"psk":%s,"handshake":%s,"compact":%s,"device_guard":%s}}},' \
	"$patch_psk" "$patch_health" "$patch_compact" "$patch_device_guard"
printf '"nodes":{"total":%s,"ok":%s,"down":%s,"unknown":%s},' \
	"$nodes_total" "$nodes_ok" "$nodes_down" "$nodes_unknown"
printf '"log":%s}\n' "$log_json"
