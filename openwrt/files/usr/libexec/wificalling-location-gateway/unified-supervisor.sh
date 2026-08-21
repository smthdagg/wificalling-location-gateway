#!/bin/sh
# Unified Gateway/WLOC lifecycle supervisor.
#
# This is intentionally a small, POSIX/busybox-compatible coordinator. The
# stable Gateway 1.7 service remains available as a rollback facade, but only
# this supervisor is enabled after migration. It owns ordering and cleanup;
# the Gateway nftables table and UDP 500/4500 handling remain owned by the
# stable Gateway init/helper and are never edited here.

set -eu

APP=wificalling-location-gateway
RUNDIR=${WLOC_UNIFIED_RUNDIR:-/var/run/$APP}
STATE=$RUNDIR/supervisor.json
PIDFILE=$RUNDIR/supervisor.pid
LOCKDIR=$RUNDIR/.lock
WLOC_INIT=${WLOC_INIT:-/etc/init.d/wloc-service}
GATEWAY_INIT=${GATEWAY_INIT:-/etc/init.d/wificalling-gateway}
REDIRECT_HELPER=${WLOC_REDIRECT_HELPER:-/usr/sbin/wloc-redirect-sync.sh}
CHECK_INTERVAL=${WLOC_SUPERVISOR_HEALTH_INTERVAL:-10}
MAX_RUNTIME_SECONDS=${WLOC_SUPERVISOR_MAX_RUNTIME:-0}
START_TIMEOUT=${WLOC_SUPERVISOR_START_TIMEOUT:-30}
case "$CHECK_INTERVAL" in ''|*[!0-9]*) CHECK_INTERVAL=10;; esac
[ "$CHECK_INTERVAL" -ge 1 ] || CHECK_INTERVAL=1
[ "$CHECK_INTERVAL" -le 60 ] || CHECK_INTERVAL=60
case "$MAX_RUNTIME_SECONDS" in ''|*[!0-9]*) MAX_RUNTIME_SECONDS=0;; esac
case "$START_TIMEOUT" in ''|*[!0-9]*) START_TIMEOUT=30;; esac
[ "$START_TIMEOUT" -ge 1 ] || START_TIMEOUT=1
[ "$START_TIMEOUT" -le 120 ] || START_TIMEOUT=120
gateway_running=0
wloc_running=0
redirect_present=0

now() { date +%s; }

json_escape() {
	printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/[[:cntrl:]]/ /g'
}

write_state() {
	phase=$1; reason=$2
	updated=$(now)
	printf '{"version":1,"phase":"%s","reason":"%s","gateway":%s,"wloc":%s,"redirect":%s,"updated_at":%s}\n' \
		"$(json_escape "$phase")" "$(json_escape "$reason")" \
		"$gateway_running" "$wloc_running" "$redirect_present" "$updated" > "$STATE"
	chmod 0600 "$STATE"
}

pid_alive() {
	pid=$1
	[ -n "$pid" ] && kill -0 "$pid" 2>/dev/null
}

read_pid() {
	[ -s "$PIDFILE" ] || return 1
	pid=$(sed -n '1p' "$PIDFILE")
	case "$pid" in ''|*[!0-9]*) return 1;; esac
	pid_alive "$pid"
}

stop_child() {
	init=$1
	[ -x "$init" ] || return 0
	"$init" stop >/dev/null 2>&1 || true
}

withdraw_redirect() {
	redirect_present=0
	[ -x "$REDIRECT_HELPER" ] && "$REDIRECT_HELPER" stop >/dev/null 2>&1 || true
}

cleanup_runtime() {
	reason=$1
	keep_gateway=${2:-1}
	state_phase=${3:-degraded_passthrough}
	withdraw_redirect
	stop_child "$WLOC_INIT"
	wloc_running=0
	if [ "$keep_gateway" -eq 1 ]; then
		# The stable Gateway is deliberately never stopped here. WLOC failure
		# is fail-open, and explicit stop/reload only withdraws WLOC-owned
		# rules; the stable Gateway table remains under its original owner.
		gateway_running=1
		write_state "$state_phase" "$reason"
	else
		gateway_running=0
		write_state "$state_phase" "$reason"
	fi
	rm -f "$PIDFILE"
}

stop_supervisor() {
	if read_pid; then
		pid=$(sed -n '1p' "$PIDFILE")
		[ "$$" = "$pid" ] || kill -TERM "$pid" 2>/dev/null || true
		# The procd-owned process receives SIGTERM as well; cleanup is also
		# idempotent here for direct upgrade/rollback invocations.
	fi
	cleanup_runtime requested_stop 1 stopped
	rmdir "$LOCKDIR" 2>/dev/null || true
}

health_ok() {
	# The supervisor only considers a child healthy when its init action
	# succeeded, the process is observable, and WLOC's root-only socket exists.
	# No network probe or unbounded command output is used in this loop.
	if command -v pgrep >/dev/null 2>&1; then
		pgrep -f '/usr/bin/sing-box run' >/dev/null 2>&1 || return 1
		pgrep -f '/usr/sbin/wloc-service' >/dev/null 2>&1 || return 1
	else
		return 1
	fi
	[ -S "${WLOC_SOCKET:-/var/run/wloc-service/control.sock}" ] || return 1
}

gateway_healthy() {
	command -v pgrep >/dev/null 2>&1 || return 1
	pgrep -f '/usr/bin/sing-box run' >/dev/null 2>&1
}

wait_for_health() {
	deadline=$(( $(now) + START_TIMEOUT ))
	while ! health_ok; do
		[ "$(now)" -lt "$deadline" ] || return 1
		sleep 1
	done
}

start_supervisor() {
	mkdir -p "$RUNDIR"
	chmod 0700 "$RUNDIR"
	if read_pid; then
		return 0
	fi
	rmdir "$LOCKDIR" 2>/dev/null || true
	if ! mkdir "$LOCKDIR" 2>/dev/null; then
		return 0
	fi
	trap 'cleanup_runtime signal 1 stopped; rmdir "$LOCKDIR" 2>/dev/null || true; exit 0' TERM INT
	echo "$$" > "$PIDFILE"
	chmod 0600 "$PIDFILE"
	gateway_running=0
	wloc_running=0
	redirect_present=0
	write_state starting passthrough

	# Legacy children are used only as a one-release adapter. Disable their
	# independent boot ownership before invoking them under this boundary.
	[ -x "$WLOC_INIT" ] && "$WLOC_INIT" disable >/dev/null 2>&1 || true
	[ -x "$GATEWAY_INIT" ] && "$GATEWAY_INIT" disable >/dev/null 2>&1 || true
	stop_child "$WLOC_INIT"

	if ! gateway_healthy; then
		if ! WLOC_SUPERVISED=1 "$GATEWAY_INIT" start >/dev/null 2>&1; then
			cleanup_runtime gateway_start_failed 0 stopped
			exit 1
		fi
	fi
	gateway_running=1
	write_state starting gateway_ready

	if ! WLOC_SUPERVISED=1 WLOC_DEFER_REDIRECT=1 WLOC_SKIP_REDIRECT=1 "$WLOC_INIT" start >/dev/null 2>&1; then
		cleanup_runtime wloc_start_failed 1
		exit 1
	fi
	wloc_running=1
	write_state passthrough children_started

	# Redirect installation is the final step. It only edits the dedicated
	# wloc_service table and policy route; it never touches Gateway tables.
	if ! wait_for_health; then
		cleanup_runtime child_health_failed 1
		exit 1
	fi
	if ! "$REDIRECT_HELPER" start >/dev/null 2>&1; then
		cleanup_runtime redirect_install_failed 1
		exit 1
	fi
	redirect_present=1
	write_state intercepting ready

	started=$(now)
	while :; do
		if ! health_ok; then
			cleanup_runtime health_failed 1
			exit 1
		fi
		if [ "$MAX_RUNTIME_SECONDS" -gt 0 ] 2>/dev/null \
			&& [ "$(($(now) - started))" -ge "$MAX_RUNTIME_SECONDS" ]; then
			cleanup_runtime test_runtime_limit 1 stopped
			exit 0
		fi
		sleep "$CHECK_INTERVAL"
	done
}

main() {
	command=${1:-start}
	case "$command" in
		start) start_supervisor ;;
		stop) stop_supervisor ;;
		reload) stop_supervisor; start_supervisor ;;
		status) [ -s "$STATE" ] && cat "$STATE" || printf '%s\n' '{"version":1,"phase":"stopped","reason":"not_running"}' ;;
		*) echo "usage: $0 {start|stop|reload|status}" >&2; exit 2 ;;
	esac
}

main "$@"
