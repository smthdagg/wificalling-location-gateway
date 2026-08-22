#!/bin/sh
# Unified Wi-Fi Calling Gateway + WLOC lifecycle supervisor.
# The two modules remain separately scoped internally, but this is their one
# product lifecycle owner. The external Gateway 1.7 repository is not read.

set -eu

APP=wificalling-location-gateway
RUNDIR=${WLOC_UNIFIED_RUNDIR:-/var/run/$APP}
STATE=$RUNDIR/supervisor.json
PIDFILE=$RUNDIR/supervisor.pid
LOCKDIR=$RUNDIR/.lock
WLOC_INIT=${WLOC_INIT:-/etc/init.d/wloc-service}
GATEWAY_INIT=${GATEWAY_INIT:-/etc/init.d/wificalling-gateway}
GATEWAY_RUNDIR=${GATEWAY_RUNDIR:-/var/run/wificalling-gateway}
GATEWAY_CONFIG=${GATEWAY_CONFIG:-$GATEWAY_RUNDIR/sing-box.json}
GATEWAY_SINGBOX_PID=${GATEWAY_SINGBOX_PID:-$GATEWAY_RUNDIR/sing-box.pid}
GATEWAY_MONITOR_PID=${GATEWAY_MONITOR_PID:-$GATEWAY_RUNDIR/monitor.pid}
REDIRECT_HELPER=${WLOC_REDIRECT_HELPER:-/usr/sbin/wloc-redirect-sync.sh}
PROFILE_REDIRECT_HELPER=${WLOC_PROFILE_REDIRECT_HELPER:-/usr/sbin/wloc-profile-redirect.sh}
WLOC_SOCKET=${WLOC_SOCKET:-/var/run/wloc-service/control.sock}
WLOC_SERVICE_PIDFILE=${WLOC_SERVICE_PIDFILE:-/var/run/wloc-service/service.pid}
WLOC_SERVICE_BIN=${WLOC_SERVICE_BIN:-/usr/sbin/wloc-service}
WLOC_REFRESH_SET_HELPER=${WLOC_REFRESH_SET_HELPER:-/usr/sbin/wloc-refresh-set.sh}
UPSTREAM_IP_FILE=${WLOC_UPSTREAM_IP_FILE:-/var/run/wloc-service/apple-upstream-ip}
PROVIDER_HELPER=${WLOC_PROVIDER_HELPER:-/usr/libexec/wificalling-location-gateway/singbox-runtime.sh}
PROFILE_PROXY_READY_FILE=${WLOC_PROFILE_PROXY_READY_FILE:-/var/run/wloc-service/profiles/.proxy-ready}
PROFILE_ACTIVATE_FILE=${WLOC_PROFILE_ACTIVATE_FILE:-/var/run/wloc-service/profiles/.activate}
PROFILE_READY_FILE=${WLOC_PROFILE_READY_FILE:-/var/run/wloc-service/profiles/.ready}
CHECK_INTERVAL=${WLOC_SUPERVISOR_HEALTH_INTERVAL:-10}
START_TIMEOUT=${WLOC_SUPERVISOR_START_TIMEOUT:-30}
case "$CHECK_INTERVAL" in ''|*[!0-9]*) CHECK_INTERVAL=10;; esac
[ "$CHECK_INTERVAL" -ge 1 ] || CHECK_INTERVAL=1
[ "$CHECK_INTERVAL" -le 60 ] || CHECK_INTERVAL=60
case "$START_TIMEOUT" in ''|*[!0-9]*) START_TIMEOUT=30;; esac
[ "$START_TIMEOUT" -ge 1 ] || START_TIMEOUT=1
[ "$START_TIMEOUT" -le 120 ] || START_TIMEOUT=120
wloc_running=0
gateway_running=0
provider_available=0
redirect_present=0

now() { date +%s; }
json_escape() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/[[:cntrl:]]/ /g'; }

write_state() {
	phase=$1; reason=$2
	printf '{"version":3,"phase":"%s","reason":"%s","gateway":%s,"wloc":%s,"provider":%s,"redirect":%s,"updated_at":%s}\n' \
		"$(json_escape "$phase")" "$(json_escape "$reason")" "$gateway_running" "$wloc_running" "$provider_available" "$redirect_present" "$(now)" > "$STATE"
	chmod 0600 "$STATE"
}

pid_alive() { [ -n "$1" ] && kill -0 "$1" 2>/dev/null; }

provider_config_path() {
	config=${WLOC_SINGBOX_CONFIG:-}
	[ -n "$config" ] || config=$(uci -q get wloc-service.main.singbox_config 2>/dev/null || true)
	[ -n "$config" ] || config=/var/run/wloc-service/sing-box.json
	printf '%s\n' "$config"
}

provider_health() {
	[ -x "$PROVIDER_HELPER" ] || return 1
	"$PROVIDER_HELPER" path >/dev/null 2>&1 || return 1
	config=$(provider_config_path)
	[ -f "$config" ] || return 1
	"$PROVIDER_HELPER" check -c "$config" >/dev/null 2>&1
}

gateway_enabled() {
	command -v uci >/dev/null 2>&1 || return 1
	enabled=$(uci -q get wificalling-gateway.main.enabled 2>/dev/null || true)
	case "$enabled" in 1|true|on|yes) return 0 ;; *) return 1 ;; esac
}

wloc_enabled() {
	command -v uci >/dev/null 2>&1 || return 0
	enabled=$(uci -q get wloc-service.main.enabled 2>/dev/null || true)
	case "$enabled" in 0|false|off|no) return 1 ;; *) return 0 ;; esac
}

product_enabled() { gateway_enabled || wloc_enabled; }

gateway_pid_matches() {
	pidfile=$1
	command=$2
	[ -s "$pidfile" ] || return 1
	pid=$(sed -n '1p' "$pidfile")
	case "$pid" in ''|*[!0-9]*) return 1;; esac
	pid_alive "$pid" || return 1
	[ -r "/proc/$pid/cmdline" ] || return 1
	cmdline=$(tr '\000' ' ' < "/proc/$pid/cmdline" 2>/dev/null)
	case "$command" in
		*.sh)
			case " $cmdline " in *" $command "*) return 0 ;; *) return 1 ;; esac
			;;
		*)
			first_arg=$(printf '%s' "$cmdline" | awk '{print $1}')
			[ "$first_arg" = "$command" ]
			;;
	esac
}

gateway_health() {
	gateway_enabled || return 0
	[ -f "$GATEWAY_CONFIG" ] || return 1
	command -v sing-box >/dev/null 2>&1 || return 1
	sing-box check -c "$GATEWAY_CONFIG" >/dev/null 2>&1 || return 1
	gateway_pid_matches "$GATEWAY_SINGBOX_PID" /usr/bin/sing-box || return 1
	gateway_pid_matches "$GATEWAY_MONITOR_PID" /usr/libexec/wificalling-gateway/monitor-loop.sh
}

start_gateway() {
	[ -x "$GATEWAY_INIT" ] || return 1
	WLOC_SUPERVISED=1 "$GATEWAY_INIT" start >/dev/null 2>&1
}

stop_gateway() {
	[ -x "$GATEWAY_INIT" ] && WLOC_SUPERVISED=1 "$GATEWAY_INIT" stop >/dev/null 2>&1 || true
	rm -f "$GATEWAY_SINGBOX_PID" "$GATEWAY_MONITOR_PID"
}

read_pid() {
	[ -s "$PIDFILE" ] || return 1
	pid=$(sed -n '1p' "$PIDFILE")
	case "$pid" in ''|*[!0-9]*) return 1;; esac
	pid_alive "$pid"
}

stop_wloc() { [ -x "$WLOC_INIT" ] && "$WLOC_INIT" stop >/dev/null 2>&1 || true; }

withdraw_redirect() {
	redirect_present=0
	[ -x "$REDIRECT_HELPER" ] && "$REDIRECT_HELPER" stop >/dev/null 2>&1 || true
	[ -x "$PROFILE_REDIRECT_HELPER" ] && "$PROFILE_REDIRECT_HELPER" stop-all >/dev/null 2>&1 || true
	rm -f "$PROFILE_PROXY_READY_FILE" "$PROFILE_ACTIVATE_FILE" "$PROFILE_READY_FILE"
}

main_service_enabled() {
	product_enabled
}

multi_profile_mode() {
	command -v uci >/dev/null 2>&1 || return 1
	profiles=$(uci -q show wloc-service 2>/dev/null | sed -n 's/^wloc-service\.[a-z0-9_]*=device$/x/p' | wc -l | tr -d ' ')
	[ "${profiles:-0}" -gt 1 ] 2>/dev/null
}

install_redirect() {
	if multi_profile_mode; then
		[ -x "$REDIRECT_HELPER" ] && "$REDIRECT_HELPER" legacy-stop >/dev/null 2>&1 || true
		[ -x "$PROFILE_REDIRECT_HELPER" ] || return 1
		"$PROFILE_REDIRECT_HELPER" route-start
		[ ! -x "$WLOC_REFRESH_SET_HELPER" ] || "$WLOC_REFRESH_SET_HELPER" >/dev/null 2>&1 || return 1
		: > "$PROFILE_ACTIVATE_FILE"
		deadline=$(( $(now) + START_TIMEOUT ))
		while [ ! -f "$PROFILE_READY_FILE" ]; do
			[ "$(now)" -lt "$deadline" ] || return 1
			sleep 1
		done
	else
		"$REDIRECT_HELPER" start
		[ ! -x "$WLOC_REFRESH_SET_HELPER" ] || "$WLOC_REFRESH_SET_HELPER" >/dev/null 2>&1 || return 1
	fi
	redirect_present=1
}

service_pid_matches() {
	[ -s "$WLOC_SERVICE_PIDFILE" ] || return 1
	service_pid=$(sed -n '1p' "$WLOC_SERVICE_PIDFILE")
	case "$service_pid" in ''|*[!0-9]*) return 1;; esac
	pid_alive "$service_pid" || return 1
	[ -r "/proc/$service_pid/cmdline" ] || return 1
	service_command=$(tr '\000' '\n' < "/proc/$service_pid/cmdline" 2>/dev/null | sed -n '1p')
	[ "$service_command" = "$WLOC_SERVICE_BIN" ]
}

health_ok() {
	if gateway_enabled; then
		gateway_health || return 1
	fi
	if wloc_enabled; then
		service_pid_matches || return 1
		[ -S "$WLOC_SOCKET" ] || return 1
		if provider_health; then
			provider_available=1
		else
			provider_available=0
			return 1
		fi
		if multi_profile_mode; then [ -f "$PROFILE_PROXY_READY_FILE" ] || return 1; fi
	else
		provider_available=0
	fi
}

wait_for_health() {
	deadline=$(( $(now) + START_TIMEOUT ))
	while ! health_ok; do
		[ "$(now)" -lt "$deadline" ] || return 1
		sleep 1
	done
}

cleanup_runtime() {
	reason=$1
	withdraw_redirect
	# Cleanup is also called by a fresh `stop` invocation, whose in-memory
	# flags are zero even though the long-running supervisor owns children.
	# Always ask both child init scripts to stop so stale Gateway/WLOC processes
	# cannot survive a service stop or a failed startup.
	stop_wloc
	stop_gateway
	rm -f "$WLOC_SERVICE_PIDFILE" "$WLOC_SOCKET" "$PIDFILE"
	wloc_running=0
	gateway_running=0
	write_state degraded_passthrough "$reason"
}

stop_supervisor() {
	# `stop` is launched as a new shell by procd. Signal the existing long-lived
	# supervisor first; otherwise cleanup would update the state file while the
	# original process continued to run and later restart its child services.
	existing_pid=
	if [ -s "$PIDFILE" ]; then
		existing_pid=$(sed -n '1p' "$PIDFILE")
	fi
	case "$existing_pid" in
		''|*[!0-9]*|"$$") ;;
		*)
			if pid_alive "$existing_pid"; then
				kill -TERM "$existing_pid" 2>/dev/null || true
				deadline=$(( $(now) + 10 ))
				while pid_alive "$existing_pid" && [ "$(now)" -lt "$deadline" ]; do sleep 1; done
			fi
			;;
	esac
	cleanup_runtime requested_stop
	rmdir "$LOCKDIR" 2>/dev/null || true
}

start_supervisor() {
	mkdir -p "$RUNDIR"; chmod 0700 "$RUNDIR"
	if read_pid; then return 0; fi
	rmdir "$LOCKDIR" 2>/dev/null || true
	mkdir "$LOCKDIR" 2>/dev/null || return 0
	trap 'cleanup_runtime signal; rmdir "$LOCKDIR" 2>/dev/null || true; exit 0' TERM INT
	echo "$$" > "$PIDFILE"; chmod 0600 "$PIDFILE"
	wloc_running=0; gateway_running=0; provider_available=0; redirect_present=0
	write_state starting passthrough
	if ! main_service_enabled; then
		cleanup_runtime disabled_by_configuration
		rmdir "$LOCKDIR" 2>/dev/null || true
		exit 0
	fi
	# The child init scripts are disabled so only this supervisor owns their
	# boot/restart lifecycle. Existing procd instances are stopped before the
	# unified start sequence to prevent duplicate sing-box processes.
	[ -x "$GATEWAY_INIT" ] && "$GATEWAY_INIT" disable >/dev/null 2>&1 || true
	[ -x "$WLOC_INIT" ] && "$WLOC_INIT" disable >/dev/null 2>&1 || true
	# Always stop any independently-started child instance before handing
	# ownership to this supervisor. The running flags only describe this
	# supervisor invocation, not a stale procd instance from a previous boot.
	stop_gateway
	stop_wloc
	if gateway_enabled; then
		if ! start_gateway; then
			cleanup_runtime gateway_start_failed
			exit 1
		fi
		gateway_running=1
		WLOC_SINGBOX_CONFIG="$GATEWAY_CONFIG"
		export WLOC_SINGBOX_CONFIG
		write_state starting gateway_started
	fi
	withdraw_redirect
	rm -f "$UPSTREAM_IP_FILE"
	if wloc_enabled; then
		if [ ! -x "$WLOC_REFRESH_SET_HELPER" ] || ! "$WLOC_REFRESH_SET_HELPER" >/dev/null 2>&1; then
			cleanup_runtime upstream_resolution_failed
			exit 1
		fi
		if gateway_enabled; then
			if ! WLOC_SINGBOX_CONFIG="$GATEWAY_CONFIG" WLOC_SUPERVISED=1 WLOC_DEFER_REDIRECT=1 WLOC_SKIP_REDIRECT=1 "$WLOC_INIT" start >/dev/null 2>&1; then
				cleanup_runtime wloc_start_failed
				exit 1
			fi
		else
			if ! WLOC_SUPERVISED=1 WLOC_DEFER_REDIRECT=1 WLOC_SKIP_REDIRECT=1 "$WLOC_INIT" start >/dev/null 2>&1; then
				cleanup_runtime wloc_start_failed
				exit 1
			fi
		fi
		wloc_running=1
		write_state starting wloc_started
	fi
	if ! wait_for_health; then cleanup_runtime wloc_health_failed; exit 1; fi
	if wloc_enabled && ! install_redirect >/dev/null 2>&1; then cleanup_runtime redirect_install_failed; exit 1; fi
	if wloc_enabled; then
		write_state intercepting ready
	else
		write_state running gateway_ready
	fi
	while :; do
		main_service_enabled || { cleanup_runtime disabled_by_configuration; exit 0; }
		if [ "$wloc_running" -eq 1 ]; then
			[ ! -x "$WLOC_REFRESH_SET_HELPER" ] || "$WLOC_REFRESH_SET_HELPER" >/dev/null 2>&1 || { cleanup_runtime upstream_refresh_failed; exit 1; }
		fi
		if ! health_ok; then cleanup_runtime health_failed; exit 1; fi
		sleep "$CHECK_INTERVAL"
	done
}

case "${1:-start}" in
	start) start_supervisor ;;
	stop) stop_supervisor ;;
	reload) stop_supervisor; start_supervisor ;;
	health) health_ok ;;
	status) [ -s "$STATE" ] && cat "$STATE" || printf '%s\n' '{"version":3,"phase":"stopped","reason":"not_running","gateway":false,"wloc":false}' ;;
	*) echo "usage: $0 {start|stop|reload|health|status}" >&2; exit 2 ;;
esac
