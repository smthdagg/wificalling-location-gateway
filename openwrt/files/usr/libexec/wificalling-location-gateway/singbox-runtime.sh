#!/bin/sh
# Resolve the small-gateway sing-box provider without downloading or copying a
# second binary. The selected executable is still started and supervised by
# this package; PassWall's running process/config is never modified.

set -eu

valid_path() {
	case "$1" in
		''|*[!A-Za-z0-9_./-]*|*/../*|*/./*) return 1 ;;
		/*) return 0 ;;
		*) return 1 ;;
	esac
}

usable() {
	candidate=$1
	valid_path "$candidate" || return 1
	[ -x "$candidate" ] || return 1
	"$candidate" version >/dev/null 2>&1
}

configured=${WLOC_SINGBOX_BIN:-}
if [ -z "$configured" ] && command -v uci >/dev/null 2>&1; then
	configured=$(uci -q get wificalling-gateway.main.singbox_bin 2>/dev/null || true)
fi

resolve() {
	if [ -n "$configured" ]; then
		usable "$configured" || return 1
		printf '%s\n' "$configured"
		return 0
	fi
	candidates=${WLOC_SINGBOX_CANDIDATES:-"/usr/bin/sing-box-tiny /usr/bin/sing-box-lite /usr/libexec/passwall/sing-box /usr/share/passwall/bin/sing-box /usr/share/passwall/bin/sing-box-tiny /usr/bin/sing-box"}
	for candidate in $candidates; do
		if usable "$candidate"; then
			printf '%s\n' "$candidate"
			return 0
		fi
	done
	return 1
}

binary=$(resolve) || {
	echo 'no usable sing-box tiny/lite/PassWall provider found' >&2
	exit 1
}

case "${1:-path}" in
	path) printf '%s\n' "$binary" ;;
	version) exec "$binary" version ;;
	check)
		[ "$#" -eq 3 ] && [ "$2" = -c ] || { echo 'usage: singbox-runtime.sh check -c CONFIG' >&2; exit 2; }
		exec "$binary" check -c "$3"
		;;
	*) echo 'usage: singbox-runtime.sh {path|version|check -c CONFIG}' >&2; exit 2 ;;
esac
