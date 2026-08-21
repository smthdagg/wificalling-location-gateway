#!/bin/sh
# Bounded standalone WLOC health projection for LuCI and the update gate.

set -eu

json_escape() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/[[:cntrl:]]/ /g'; }
now=$(date +%s)
file_age() {
	file=$1
	if [ -f "$file" ] && date -r "$file" +%s >/dev/null 2>&1; then
		echo $((now - $(date -r "$file" +%s)))
	else
		echo -1
	fi
}

wloc_pid=$(pgrep -f '/usr/sbin/wloc-service' 2>/dev/null | head -n 1 || true)
wloc_running=0; [ -n "$wloc_pid" ] && wloc_running=1
wloc_socket=0; [ -S /var/run/wloc-service/control.sock ] && wloc_socket=1
wloc_status=/var/run/wloc-service/status.json
wloc_status_fresh=0; wloc_phase=unknown; wloc_exit=unknown; wloc_geo=unknown; wloc_error=null
if [ -f "$wloc_status" ]; then
	age=$(file_age "$wloc_status")
	[ "$age" -ge 0 ] && [ "$age" -le 120 ] && wloc_status_fresh=1
	wloc_phase=$(sed -n 's/.*"service_phase"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$wloc_status" | head -n 1)
	[ -n "$wloc_phase" ] || wloc_phase=unknown
	wloc_exit=$(grep -A5 '"exit"' "$wloc_status" | sed -n 's/.*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
	[ -n "$wloc_exit" ] || wloc_exit=unknown
	wloc_geo=$(grep -A10 '"geo":' "$wloc_status" | sed -n 's/.*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
	[ -n "$wloc_geo" ] || wloc_geo=unknown
	wloc_err=$(sed -n 's/.*"last_error"[[:space:]]*:[[:space:]]*\([^,}]*\).*/\1/p' "$wloc_status" | head -n 1)
	[ -n "$wloc_err" ] && wloc_error=$(json_escape "$wloc_err")
fi

HELPER=${WLOC_PROVIDER_HELPER:-/usr/libexec/wificalling-location-gateway/singbox-runtime.sh}
provider_bin=$([ -x "$HELPER" ] && "$HELPER" path 2>/dev/null || true)
provider_available=0; provider_valid=0
[ -n "$provider_bin" ] && provider_available=1
config_path=$(uci -q get wloc-service.main.singbox_config 2>/dev/null || echo /var/run/wloc-service/sing-box.json)
config_present=0; config_valid=0; config_age=-1
if [ -f "$config_path" ]; then
	config_present=1; config_age=$(file_age "$config_path")
	[ -n "$provider_bin" ] && "$provider_bin" check -c "$config_path" >/dev/null 2>&1 && config_valid=1
fi
[ "$provider_available" -eq 1 ] && [ "$config_valid" -eq 1 ] && provider_valid=1

nft_rules=0
if command -v nft >/dev/null 2>&1; then
	nft list table inet wloc_service >/dev/null 2>&1 && nft_rules=1 || true
fi

profiles='[]'
if [ -x /usr/sbin/wloc-profile-status.sh ]; then
	profile_json=$(/usr/sbin/wloc-profile-status.sh 2>/dev/null || true)
	profiles=$(printf '%s\n' "$profile_json" | sed -n 's/^{"profiles":\(.*\)}$/\1/p')
	[ -n "$profiles" ] || profiles='[]'
fi

printf '{"generated_at":%s,' "$now"
printf '"services":{"wloc":{"running":%s,"socket":%s,"status_fresh":%s,"phase":"%s","exit":"%s","geo":"%s","last_error":%s},' \
	"$wloc_running" "$wloc_socket" "$wloc_status_fresh" "$wloc_phase" "$wloc_exit" "$wloc_geo" "$wloc_error"
printf '"provider":{"available":%s,"valid":%s,"config_present":%s,"config_valid":%s,"config_age":%s},' \
	"$provider_available" "$provider_valid" "$config_present" "$config_valid" "$config_age"
printf '"redirect":{"table_present":%s,"rules":%s}},' "$nft_rules" "$nft_rules"
printf '"nodes":{"total":0,"ok":0,"down":0,"unknown":0},"profiles":%s}\n' "$profiles"
