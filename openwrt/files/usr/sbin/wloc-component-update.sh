#!/bin/sh
# Transactional updater for the unified Gateway/WLOC component.
#
# The package is validated before the first mutation. A known-good package and
# configuration snapshot are retained until restart and health validation pass;
# a failed activation or interrupted transaction can therefore be recovered.

set -eu

ROOT=${WLOC_UPDATE_ROOT:-/}
STATE_DIR=${WLOC_UPDATE_STATE_DIR:-/var/lib/wificalling-location-gateway/update}
OPKG=${WLOC_UPDATE_OPKG:-}
if [ -z "$OPKG" ]; then
	OPKG=$(command -v opkg 2>/dev/null || true)
	[ -n "$OPKG" ] || OPKG=/usr/bin/opkg
fi
SUPERVISOR=${WLOC_UPDATE_SUPERVISOR:-/etc/init.d/wificalling-location-gateway}
HEALTH=${WLOC_UPDATE_HEALTH:-/usr/sbin/wloc-health.sh}
HEALTH_TIMEOUT=${WLOC_UPDATE_HEALTH_TIMEOUT:-30}
STATUS=$STATE_DIR/status.json
TXN=$STATE_DIR/transaction
LOCK=$STATE_DIR/.lock
WORK=
case "$HEALTH_TIMEOUT" in
	''|*[!0-9]*) HEALTH_TIMEOUT=30 ;;
esac
[ "$HEALTH_TIMEOUT" -ge 1 ] || HEALTH_TIMEOUT=1
[ "$HEALTH_TIMEOUT" -le 120 ] || HEALTH_TIMEOUT=120

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
	chmod 0700 "$STATE_DIR"
	temporary="$STATE_DIR/status.tmp.$$"
	printf '{"version":1,"phase":"%s","reason":"%s","target_version":"%s","current_version":"%s","updated_at":%s}\n' \
		"$phase" "$reason" "$target" "$current" "$(now)" > "$temporary"
	chmod 0600 "$temporary"
	mv -f "$temporary" "$STATUS"
}

cleanup_work() {
	if [ -n "${WORK:-}" ]; then
		rm -rf "$WORK"
		WORK=
	fi
}

cleanup_lock() {
	cleanup_work
	rm -f "$LOCK/pid"
	rmdir "$LOCK" 2>/dev/null || true
}

health_check() {
	health_report=$($HEALTH 2>/dev/null) || return 1
	printf '%s\n' "$health_report" | grep -F '"wloc":{"running":1,"socket":1,"status_fresh":1' >/dev/null || return 1
	printf '%s\n' "$health_report" | grep -F '"gateway":{"running":1,"monitor":1,"singbox":1,"config_present":1,"config_valid":1' >/dev/null || return 1
}

wait_for_health() {
	deadline=$(( $(now) + HEALTH_TIMEOUT ))
	while ! health_check; do
		[ "$(now)" -lt "$deadline" ] || return 1
		sleep 1
	done
}

trap 'cleanup_work' EXIT HUP INT TERM

acquire_lock() {
	mkdir -p "$STATE_DIR"
	chmod 0700 "$STATE_DIR"
	if ! mkdir "$LOCK" 2>/dev/null; then
		owner=$(cat "$LOCK/pid" 2>/dev/null || true)
		case "$owner" in
			''|*[!0-9]*) die 'component update lock is stale or unreadable; inspect before recovery' ;;
			esac
		if kill -0 "$owner" 2>/dev/null; then
			die 'component update is already in progress'
		fi
		rm -f "$LOCK/pid"
		rmdir "$LOCK" 2>/dev/null || die 'component update lock could not be reclaimed safely'
		mkdir "$LOCK" 2>/dev/null || die 'component update is already in progress'
	fi
	printf '%s\n' "$$" > "$LOCK/pid"
	chmod 0600 "$LOCK/pid"
	trap 'cleanup_lock' EXIT HUP INT TERM
}

field() {
	key=$1
	file=$2
	sed -n "s/^${key}:[[:space:]]*//p" "$file" | head -n 1
}

sha256_file() {
	command -v sha256sum >/dev/null 2>&1 || die 'sha256sum is required for update verification'
	sha256sum "$1" | awk '{print $1}'
}

verify_manifest() {
	package=$1
	control_archive=$2
	data_archive=$3
	manifest=${WLOC_UPDATE_MANIFEST:-$package.manifest}
	signature=${WLOC_UPDATE_SIGNATURE:-$package.sig}
	public_key=${WLOC_UPDATE_PUBLIC_KEY:-/etc/wificalling-location-gateway/update.pub}
	usign=${WLOC_UPDATE_USIGN:-/usr/bin/usign}
	[ -f "$manifest" ] && [ ! -L "$manifest" ] || die 'update manifest is required'
	[ -f "$signature" ] && [ ! -L "$signature" ] || die 'update manifest signature is required'
	[ -f "$public_key" ] && [ ! -L "$public_key" ] || die 'update verification key is required'
	[ -x "$usign" ] || die 'usign is required for update verification'
	[ "$(wc -c < "$manifest" | tr -d ' ')" -le 4096 ] || die 'update manifest is too large'
	format=$(field Format "$manifest")
	manifest_package=$(field Package "$manifest")
	manifest_version=$(field Version "$manifest")
	manifest_architecture=$(field Architecture "$manifest")
	manifest_package_sha256=$(field Package-SHA256 "$manifest")
	manifest_control_sha256=$(field Control-SHA256 "$manifest")
	manifest_data_sha256=$(field Data-SHA256 "$manifest")
	[ "$format" = 'wfc-update-manifest/v1' ] || die 'update manifest format is invalid'
	[ "$manifest_package" = "$(field Package "$work/control/control")" ] || die 'update manifest package mismatch'
	[ "$manifest_version" = "$(field Version "$work/control/control")" ] || die 'update manifest version mismatch'
	[ "$manifest_architecture" = "$(field Architecture "$work/control/control")" ] || die 'update manifest architecture mismatch'
	printf '%s\n' "$manifest_package_sha256" | grep -Eq '^[0-9a-fA-F]{64}$' || die 'update manifest package hash is invalid'
	printf '%s\n' "$manifest_control_sha256" | grep -Eq '^[0-9a-fA-F]{64}$' || die 'update manifest control hash is invalid'
	printf '%s\n' "$manifest_data_sha256" | grep -Eq '^[0-9a-fA-F]{64}$' || die 'update manifest data hash is invalid'
	[ "$manifest_package_sha256" = "$(sha256_file "$package")" ] || die 'update package hash mismatch'
	[ "$manifest_control_sha256" = "$(sha256_file "$control_archive")" ] || die 'update control archive hash mismatch'
	[ "$manifest_data_sha256" = "$(sha256_file "$data_archive")" ] || die 'update data archive hash mismatch'
	"$usign" -V -p "$public_key" -m "$manifest" -x "$signature" >/dev/null 2>&1 \
		|| die 'update manifest signature is invalid'
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
	printf '%s\n' "$1" | sed 's/-r/-/' | awk -F '[.-]' '{printf "%03d%03d%03d%03d", $1+0, $2+0, $3+0, $4+0}'
}

validate_version() {
	printf '%s\n' "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+-[0-9A-Za-z]+$'
}

free_kb() {
	if [ -n "${WLOC_UPDATE_FREE_KB:-}" ]; then
		printf '%s\n' "$WLOC_UPDATE_FREE_KB"
		return
	fi
	state_free=$(df -Pk "$STATE_DIR" | awk 'NR == 2 { print $4; exit }')
	tmp_free=$(df -Pk "${TMPDIR:-/tmp}" | awk 'NR == 2 { print $4; exit }')
	case "$state_free:$tmp_free" in
		*[!0-9:]*|:*) return 1 ;;
	esac
	[ "$state_free" -le "$tmp_free" ] && printf '%s\n' "$state_free" || printf '%s\n' "$tmp_free"
}

package_info() {
	package=$1
	work=$2
	[ -f "$package" ] && [ ! -L "$package" ] || die 'update package must be a regular local file'
	if [ "${WLOC_UPDATE_ALLOW_ANY_SOURCE:-0}" != 1 ]; then
		stage_root=$(readlink -f /tmp/wloc-update 2>/dev/null || true)
		incoming_root=$(readlink -f "$STATE_DIR/incoming" 2>/dev/null || true)
		package_real=$(readlink -f "$package" 2>/dev/null || true)
		case "$package_real" in
			"$stage_root"/*|"$incoming_root"/*) ;;
			*) die 'update source must be under the local update staging directory' ;;
		esac
	fi
	archive_safe "$package" || die 'update package archive is unsafe or corrupt'
	tar -tzf "$package" | awk '$0 == "./control.tar.gz" || $0 == "control.tar.gz" { found=1 } END { exit found ? 0 : 1 }' \
		|| die 'update package lacks control archive'
	tar -tzf "$package" | awk '$0 == "./data.tar.gz" || $0 == "data.tar.gz" { found=1 } END { exit found ? 0 : 1 }' \
		|| die 'update package lacks data archive'
mkdir -p "$work/control"
tar -xOzf "$package" control.tar.gz > "$work/control.tar.gz" 2>/dev/null \
		|| tar -xOzf "$package" ./control.tar.gz > "$work/control.tar.gz"
	archive_safe "$work/control.tar.gz" || die 'control archive is unsafe or corrupt'
	tar -xzf "$work/control.tar.gz" -C "$work/control" ./control
	control="$work/control/control"
	name=$(field Package "$control")
	version=$(field Version "$control")
	architecture=$(field Architecture "$control")
	product=$(field X-WFC-Product "$control")
	gateway=$(field X-WFC-Gateway "$control")
	api=$(field X-WFC-Wloc-Api "$control")
	tar -xOzf "$package" data.tar.gz > "$work/data.tar.gz" 2>/dev/null \
		|| tar -xOzf "$package" ./data.tar.gz > "$work/data.tar.gz"
	archive_safe "$work/data.tar.gz" || die 'data archive is unsafe or corrupt'
	verify_manifest "$package" "$work/control.tar.gz" "$work/data.tar.gz"
	if [ -z "$product" ] || [ -z "$gateway" ] || [ -z "$api" ]; then
		compatibility=$(tar -xOf "$work/data.tar.gz" ./usr/share/wificalling-location-gateway/compatibility 2>/dev/null || true)
		[ -n "$product" ] || product=$(printf '%s\n' "$compatibility" | sed -n 's/^X-WFC-Product:[[:space:]]*//p')
		[ -n "$gateway" ] || gateway=$(printf '%s\n' "$compatibility" | sed -n 's/^X-WFC-Gateway:[[:space:]]*//p')
		[ -n "$api" ] || api=$(printf '%s\n' "$compatibility" | sed -n 's/^X-WFC-Wloc-Api:[[:space:]]*//p')
	fi
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
	tar -tzf "$work/data.tar.gz" | awk '
		/^\./ { next }
		{ bad=1 }
		END { exit bad ? 1 : 0 }
	' || die 'data archive contains an invalid path'
	printf '%s\n' "$version"
}

restore_configs() {
	for config in wloc-service wificalling-gateway; do
		if [ -f "$TXN/config.$config" ]; then
			mkdir -p "$ROOT/etc/config"
			cp -p "$TXN/config.$config" "$ROOT/etc/config/$config" || return 1
		elif [ -f "$TXN/config.$config.absent" ]; then
			rm -f "$ROOT/etc/config/$config" || return 1
		fi
	done
}

rollback_transaction() {
	reason=${1:-health_check_failed}
	old_version=$(cat "$TXN/current.version" 2>/dev/null || true)
	target_version=$(cat "$TXN/target.version" 2>/dev/null || true)
	write_status rolling_back "$reason" "$target_version" "$old_version"
	rollback_ok=1
	if [ -f "$TXN/rollback.ipk" ]; then
		"$OPKG" --force-downgrade install "$TXN/rollback.ipk" >/dev/null 2>&1 || rollback_ok=0
	else
		rollback_ok=0
	fi
	restore_configs || rollback_ok=0
	if [ "$rollback_ok" -eq 1 ]; then
		"$SUPERVISOR" restart >/dev/null 2>&1 || rollback_ok=0
		wait_for_health || rollback_ok=0
	fi
	if [ "$rollback_ok" -eq 1 ]; then
		if [ -f "$TXN/rollback.ipk" ]; then
			cp -p "$TXN/rollback.ipk" "$STATE_DIR/current.ipk" || rollback_ok=0
		fi
		if [ "$rollback_ok" -eq 1 ] && [ -n "$old_version" ]; then
			printf '%s\n' "$old_version" > "$STATE_DIR/current.version" || rollback_ok=0
		fi
	fi
	if [ "$rollback_ok" -eq 1 ]; then
		write_status rolled_back "$reason" "$target_version" "$old_version"
		rm -rf "$TXN"
		return 0
	else
		write_status rollback_failed rollback_package_install_failed "$target_version" "$old_version"
		# Keep the transaction and rollback package so an operator can repair
		# storage/opkg/service state and retry `recover` without losing the only
		# known-good restoration point.
		return 1
	fi
}

preflight() {
	package=$1
	mkdir -p "$STATE_DIR"
	chmod 0700 "$STATE_DIR"
	mkdir -m 0700 -p "$STATE_DIR/incoming"
	WORK=$(mktemp -d "${TMPDIR:-/tmp}/wloc-update-check.XXXXXX")
	version=$(package_info "$package" "$WORK")
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
	cleanup_work
}

apply_update() {
	package=$1
	acquire_lock
	[ ! -e "$TXN" ] || die 'an interrupted update must be recovered before another update'
	mkdir -m 0700 -p "$STATE_DIR/incoming"
	WORK=$(mktemp -d "${TMPDIR:-/tmp}/wloc-update-check.XXXXXX")
	version=$(package_info "$package" "$WORK")
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
	mkdir -m 0700 -p "$TXN"
	chmod 0700 "$TXN"
	printf '%s\n' "$version" > "$TXN/target.version"
	printf '%s\n' "$current" > "$TXN/current.version"
	cp -p "$rollback_package" "$TXN/rollback.ipk"
	chmod 0600 "$TXN/rollback.ipk"
	for config in wloc-service wificalling-gateway; do
		if [ -f "$ROOT/etc/config/$config" ]; then
			cp -p "$ROOT/etc/config/$config" "$TXN/config.$config"
			chmod 0600 "$TXN/config.$config"
		else
			: > "$TXN/config.$config.absent"
		fi
	done
	write_status applying validating "$version" "$current"
	"$OPKG" install "$package" >/dev/null 2>&1 || {
		rollback_transaction package_install_failed
		exit 1
	}
	if ! restore_configs; then
		rollback_transaction config_restore_failed
		exit 1
	fi
	write_status installed awaiting_health "$version" "$current"
	if [ "${WLOC_UPDATE_INTERRUPT_AFTER_INSTALL:-0}" = 1 ]; then
		write_status interrupted power_loss_simulation "$version" "$current"
		exit 75
	fi
	"$SUPERVISOR" restart >/dev/null 2>&1 || {
		rollback_transaction restart_failed
		exit 1
	}
	if ! wait_for_health; then
		rollback_transaction health_check_failed
		exit 1
	fi
	if ! cp -p "$package" "$STATE_DIR/current.ipk" \
		|| ! chmod 0600 "$STATE_DIR/current.ipk" \
		|| ! printf '%s\n' "$version" > "$STATE_DIR/current.version"; then
		rollback_transaction state_commit_failed
		exit 1
	fi
	write_status applied ready "$version" "$version"
	rm -rf "$TXN"
	cleanup_work
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
