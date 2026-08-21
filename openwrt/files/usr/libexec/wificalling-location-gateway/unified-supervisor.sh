#!/bin/sh
# Standalone WLOC lifecycle supervisor.
# Owns WLOC, the optional sing-box provider, and WLOC-only redirect objects.
# It never calls or reads another service's configuration.

set -eu

APP=wificalling-location-gateway
RUNDIR=${WLOC_UNIFIED_RUNDIR:-/var/run/$APP}
STATE=$RUNDIR/supervisor.json
PIDFILE=$RUNDIR/supervisor.pid
LOCKDIR=$RUNDIR/.lock
WLOC_INIT=${WLOC_INIT:-/etc/init.d/wloc-service}
REDIRECT_HELPER=${WLOC_REDIRECT_HELPER:-/usr/sbin/wloc-redirect-sync.sh}
PROFILE_REDIRECT_HELPER=${WLOC_PROFILE_REDIRECT_HELPER:-/usr/sbin/wloc-profile-redirect.sh}
WLOC_SOCKET=${WLOC_SOCKET:-/var/run/wloc-service/control.sock}
WLOC_SERVICE_PIDFILE=${WLOC_SERVICE_PIDFILE:-/var/run/wloc-service/service.pid}
WLOC_SERVICE_BIN=${WLOC_SERVICE_BIN:-/usr/sbin/wloc-service}
WLOC_REFRESH_SET_HELPER=${WLOC_REFRESH_SET_HELPER:-/usr/sbin/wloc-refresh-set.sh}
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
provider_available=0
redirect_present=0

now() { date +%s; }
json_escape() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/[[:cntrl:]]/ /g'; }

write_state() {
	phase=$1; reason=$2
	printf '{"version":2,"phase":"%s","reason":"%s","wloc":%s,"provider":%s,"redirect":%s,"updated_at":%s}\n' \
		"$(json_escape "$phase")" "$(json_escape "$reason")" "$wloc_running" "$provider_available" "$redirect_present" "$(now)" > "$STATE"
	chmod 0600 "$STATE"
}

pid_alive() { [ -n "$1" ] && kill -0 "$1" 2>/dev/null; }

provider_config_path() {
	config=$(uci -q get wloc-service.main.singbox_config 2>/dev/null || true)
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
		[ ! -x "$WLOC_REFRESH_SET_HELPER" ] || "$WLOC_REFRESH_SET_HELPER" >/dev/null 2>&1 || true
		: > "$PROFILE_ACTIVATE_FILE"
		deadline=$(( $(now) + START_TIMEOUT ))
		while [ ! -f "$PROFILE_READY_FILE" ]; do
			[ "$(now)" -lt "$deadline" ] || return 1
			sleep 1
		done
	else
		"$REDIRECT_HELPER" start
		[ ! -x "$WLOC_REFRESH_SET_HELPER" ] || "$WLOC_REFRESH_SET_HELPER" >/dev/null 2>&1 || true
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
	service_pid_matches || return 1
	[ -S "$WLOC_SOCKET" ] || return 1
	if provider_health; then
		provider_available=1
	else
		provider_available=0
		return 1
	fi
	if multi_profile_mode; then [ -f "$PROFILE_PROXY_READY_FILE" ] || return 1; fi
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
	stop_wloc
	rm -f "$WLOC_SERVICE_PIDFILE" "$WLOC_SOCKET" "$PIDFILE"
	wloc_running=0
	write_state degraded_passthrough "$reason"
}

stop_supervisor() {
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
	wloc_running=0; provider_available=0; redirect_present=0
	write_state starting passthrough
	provider_health && provider_available=1 || true
	[ -x "$WLOC_INIT" ] && "$WLOC_INIT" disable >/dev/null 2>&1 || true
	stop_wloc
	withdraw_redirect
	if ! WLOC_SUPERVISED=1 WLOC_DEFER_REDIRECT=1 WLOC_SKIP_REDIRECT=1 "$WLOC_INIT" start >/dev/null 2>&1; then
		cleanup_runtime wloc_start_failed
		exit 1
	fi
	wloc_running=1
	write_state starting wloc_started
	if ! wait_for_health; then cleanup_runtime wloc_health_failed; exit 1; fi
	if ! install_redirect >/dev/null 2>&1; then cleanup_runtime redirect_install_failed; exit 1; fi
	write_state intercepting ready
	while :; do
		[ ! -x "$WLOC_REFRESH_SET_HELPER" ] || "$WLOC_REFRESH_SET_HELPER" >/dev/null 2>&1 || true
		if ! health_ok; then cleanup_runtime health_failed; exit 1; fi
		sleep "$CHECK_INTERVAL"
	done
}

case "${1:-start}" in
	start) start_supervisor ;;
	stop) stop_supervisor ;;
	reload) stop_supervisor; start_supervisor ;;
	health) health_ok ;;
	status) [ -s "$STATE" ] && cat "$STATE" || printf '%s\n' '{"version":2,"phase":"stopped","reason":"not_running"}' ;;
	*) echo "usage: $0 {start|stop|reload|health|status}" >&2; exit 2 ;;
esac
