#!/bin/sh
# Transactional updater for the unified Gateway/WLOC component.
#
# The package is validated before the first mutation. A known-good package and
# configuration snapshot are retained until restart and health validation pass;
# a failed activation or interrupted transaction can therefore be recovered.

set -eu

ROOT=${WLOC_UPDATE_ROOT:-/}
STATE_DIR=${WLOC_UPDATE_STATE_DIR:-/var/lib/wificalling-location-gateway/update}
OPKG=${WLOC_UPDATE_OPKG:-/usr/bin/opkg}
SUPERVISOR=${WLOC_UPDATE_SUPERVISOR:-/etc/init.d/wificalling-location-gateway}
HEALTH=${WLOC_UPDATE_HEALTH:-/usr/sbin/wloc-health.sh}
STATUS=$STATE_DIR/status.json
TXN=$STATE_DIR/transaction
LOCK=$STATE_DIR/.lock

die() {
	printf '%s\n' "$1" >&2
	exit 1
}

now() { date +%s; }

write_status() {
	phase=$1
	reason=$2
	target=${3:-}
	current=${4:-}
	mkdir -p "$STATE_DIR"
	temporary="$STATE_DIR/status.tmp.$$"
	printf '{"version":1,"phase":"%s","reason":"%s","target_version":"%s","current_version":"%s","updated_at":%s}\n' \
		"$phase" "$reason" "$target" "$current" "$(now)" > "$temporary"
	chmod 0600 "$temporary"
	mv -f "$temporary" "$STATUS"
}

cleanup_lock() {
	rmdir "$LOCK" 2>/dev/null || true
}

acquire_lock() {
	mkdir -p "$STATE_DIR"
	if ! mkdir "$LOCK" 2>/dev/null; then
		die 'component update is already in progress'
	fi
	trap 'cleanup_lock' EXIT HUP INT TERM
}

field() {
	key=$1
	file=$2
	sed -n "s/^${key}:[[:space:]]*//p" "$file" | head -n 1
}

archive_safe() {
	archive=$1
	tar -tzf "$archive" >/dev/null 2>&1 || return 1
	tar -tzf "$archive" | awk '
		/^\// || /(^|\/)\.\.($|\/)/ { bad=1 }
		END { exit bad ? 1 : 0 }
	'
}

version_key() {
	printf '%s\n' "$1" | awk -F '[.-]' '{printf "%03d%03d%03d%03d", $1+0, $2+0, $3+0, $4+0}'
}

validate_version() {
	case "$1" in
		[0-9]*.[0-9]*.[0-9]*-[0-9A-Za-z]*) return 0 ;;
		*) return 1 ;;
	esac
}

free_kb() {
	if [ -n "${WLOC_UPDATE_FREE_KB:-}" ]; then
		printf '%s\n' "$WLOC_UPDATE_FREE_KB"
		return
	fi
	df -Pk "$STATE_DIR" | awk 'NR == 2 { print $4; exit }'
}

package_info() {
	package=$1
	work=$2
	[ -f "$package" ] && [ ! -L "$package" ] || die 'update package must be a regular local file'
	if [ "${WLOC_UPDATE_ALLOW_ANY_SOURCE:-0}" != 1 ]; then
		case "$package" in
			/tmp/wloc-update/*|$STATE_DIR/incoming/*) ;;
			*) die 'update source must be under the local update staging directory' ;;
		esac
	fi
	archive_safe "$package" || die 'update package archive is unsafe or corrupt'
	tar -tzf "$package" | grep -Fx './control.tar.gz' >/dev/null || die 'update package lacks control archive'
	tar -tzf "$package" | grep -Fx './data.tar.gz' >/dev/null || die 'update package lacks data archive'
	mkdir -p "$work/control"
	tar -xzf "$package" -C "$work" ./control.tar.gz
	archive_safe "$work/control.tar.gz" || die 'control archive is unsafe or corrupt'
	tar -xzf "$work/control.tar.gz" -C "$work/control" ./control
	control="$work/control/control"
	name=$(field Package "$control")
	version=$(field Version "$control")
	architecture=$(field Architecture "$control")
	product=$(field X-WFC-Product "$control")
	gateway=$(field X-WFC-Gateway "$control")
	api=$(field X-WFC-Wloc-Api "$control")
	case "$name" in
		wificalling-location-gateway|luci-app-wificalling-location-gateway) ;;
		*) die 'update package identity is not the unified product' ;;
	esac
	validate_version "$version" || die 'update package version is invalid'
	[ "$product" = 'wificalling-location-gateway/v2' ] || die 'update package compatibility metadata is missing'
	[ "$gateway" = '1.7' ] || die 'update package requires an incompatible Gateway major version'
	[ "$api" = 'wloc.service/v2' ] || die 'update package requires an incompatible WLOC API'
	if [ "$architecture" != all ]; then
		expected=${WLOC_UPDATE_ARCHITECTURE:-}
		if [ -z "$expected" ] && [ -x "$OPKG" ]; then
			expected=$($OPKG print-architecture 2>/dev/null | awk '$2 != "all" { print $2; exit }')
		fi
		[ -n "$expected" ] && [ "$architecture" = "$expected" ] || die 'update package architecture is incompatible'
	fi
	tar -xzf "$package" -C "$work" ./data.tar.gz
	archive_safe "$work/data.tar.gz" || die 'data archive is unsafe or corrupt'
	tar -tzf "$work/data.tar.gz" | awk '
		/^\./ { next }
		{ bad=1 }
		END { exit bad ? 1 : 0 }
	' && true
	printf '%s\n' "$version"
}

restore_configs() {
	for config in wloc-service wificalling-gateway; do
		if [ -f "$TXN/config.$config" ]; then
			mkdir -p "$ROOT/etc/config"
			cp -p "$TXN/config.$config" "$ROOT/etc/config/$config"
		elif [ -f "$TXN/config.$config.absent" ]; then
			rm -f "$ROOT/etc/config/$config"
		fi
	done
}

rollback_transaction() {
	reason=${1:-health_check_failed}
	old_version=$(cat "$TXN/current.version" 2>/dev/null || true)
	target_version=$(cat "$TXN/target.version" 2>/dev/null || true)
	write_status rolling_back "$reason" "$target_version" "$old_version"
	if [ -f "$TXN/rollback.ipk" ]; then
		"$OPKG" install "$TXN/rollback.ipk" >/dev/null 2>&1 || true
	fi
	restore_configs
	if [ -n "$old_version" ]; then
		printf '%s\n' "$old_version" > "$STATE_DIR/current.version"
	fi
	if [ -f "$TXN/rollback.ipk" ]; then
		cp -p "$TXN/rollback.ipk" "$STATE_DIR/current.ipk"
	fi
	write_status rolled_back "$reason" "$target_version" "$old_version"
	rm -rf "$TXN"
}

preflight() {
	package=$1
	work=$(mktemp -d "${TMPDIR:-/tmp}/wloc-update-check.XXXXXX")
	version=$(package_info "$package" "$work")
	bytes=$(wc -c < "$package" | tr -d ' ')
	available=$(free_kb)
	case "$available" in ''|*[!0-9]*) die 'free-space check is unavailable' ;; esac
	required=$(( (bytes / 1024) + 2048 ))
	[ "$available" -ge "$required" ] || die 'insufficient free space for a transactional update'
	current=$(cat "$STATE_DIR/current.version" 2>/dev/null || true)
	if [ -n "$current" ] && validate_version "$current"; then
		if [ "$(version_key "$version")" -lt "$(version_key "$current")" ] \
			&& [ "${WLOC_UPDATE_ALLOW_DOWNGRADE:-0}" != 1 ]; then
			die 'downgrade requires explicit authorization'
		fi
	fi
	printf '{"ok":true,"target_version":"%s","current_version":"%s","free_kb":%s}\n' "$version" "$current" "$available"
	rm -rf "$work"
}

apply_update() {
	package=$1
	acquire_lock
	[ ! -e "$TXN" ] || die 'an interrupted update must be recovered before another update'
	work=$(mktemp -d "${TMPDIR:-/tmp}/wloc-update-check.XXXXXX")
	version=$(package_info "$package" "$work")
	bytes=$(wc -c < "$package" | tr -d ' ')
	available=$(free_kb)
	case "$available" in ''|*[!0-9]*) die 'free-space check is unavailable' ;; esac
	required=$(( (bytes / 1024) + 2048 ))
	[ "$available" -ge "$required" ] || die 'insufficient free space for a transactional update'
	current=$(cat "$STATE_DIR/current.version" 2>/dev/null || true)
	if [ -n "$current" ] && validate_version "$current" \
		&& [ "$(version_key "$version")" -lt "$(version_key "$current")" ] \
		&& [ "${WLOC_UPDATE_ALLOW_DOWNGRADE:-0}" != 1 ]; then
		die 'downgrade requires explicit authorization'
	fi
	rollback_package=${WLOC_UPDATE_CURRENT_PACKAGE:-$STATE_DIR/current.ipk}
	[ -f "$rollback_package" ] && [ ! -L "$rollback_package" ] || die 'known-good rollback package is required'
	mkdir -p "$TXN"
	printf '%s\n' "$version" > "$TXN/target.version"
	printf '%s\n' "$current" > "$TXN/current.version"
	cp -p "$rollback_package" "$TXN/rollback.ipk"
	for config in wloc-service wificalling-gateway; do
		if [ -f "$ROOT/etc/config/$config" ]; then
			cp -p "$ROOT/etc/config/$config" "$TXN/config.$config"
		else
			: > "$TXN/config.$config.absent"
		fi
	done
	write_status applying validating "$version" "$current"
	"$OPKG" install "$package" >/dev/null 2>&1 || {
		rollback_transaction package_install_failed
		exit 1
	}
	restore_configs
	write_status installed awaiting_health "$version" "$current"
	if [ "${WLOC_UPDATE_INTERRUPT_AFTER_INSTALL:-0}" = 1 ]; then
		write_status interrupted power_loss_simulation "$version" "$current"
		exit 75
	fi
	"$SUPERVISOR" restart >/dev/null 2>&1 || {
		rollback_transaction restart_failed
		exit 1
	}
	if ! "$HEALTH" >/dev/null 2>&1; then
		rollback_transaction health_check_failed
		exit 1
	fi
	cp -p "$package" "$STATE_DIR/current.ipk"
	chmod 0600 "$STATE_DIR/current.ipk"
	printf '%s\n' "$version" > "$STATE_DIR/current.version"
	write_status applied ready "$version" "$version"
	rm -rf "$TXN" "$work"
}

recover_update() {
	acquire_lock
	[ -d "$TXN" ] || die 'no interrupted update transaction is present'
	rollback_transaction interrupted_transaction
}

case "${1:-status}" in
	preflight) [ "$#" -eq 2 ] || die 'usage: preflight PACKAGE'; preflight "$2" ;;
	apply) [ "$#" -eq 2 ] || die 'usage: apply PACKAGE'; apply_update "$2" ;;
	recover) recover_update ;;
	status) [ -s "$STATUS" ] && cat "$STATUS" || printf '%s\n' '{"version":1,"phase":"unknown","reason":"no_update_state"}' ;;
	*) die 'usage: {preflight|apply|recover|status} PACKAGE' ;;
esac
