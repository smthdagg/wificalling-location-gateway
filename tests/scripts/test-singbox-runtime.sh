#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
helper="$repo_root/openwrt/files/usr/libexec/wificalling-location-gateway/singbox-runtime.sh"
gateway_init="$repo_root/openwrt/files/etc/init.d/wificalling-gateway"
wloc_init="$repo_root/openwrt/files/etc/init.d/wloc-service"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-singbox-runtime.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

sh -n "$helper" "$gateway_init" "$wloc_init"
grep -F 'singbox-runtime.sh' "$gateway_init" "$wloc_init" >/dev/null
grep -F 'WLOC_SINGBOX_BIN' "$wloc_init" >/dev/null
if grep -F '/usr/bin/sing-box check' "$gateway_init" >/dev/null; then
	echo 'Gateway must use the resolved sing-box provider' >&2
	exit 1
fi

cat > "$tmp/sing-box-tiny" <<'EOF'
#!/bin/sh
case "${1:-}" in
  version) echo 'sing-box version 1.12.0-tiny'; exit 0 ;;
  check) exit 0 ;;
  *) exit 0 ;;
esac
EOF
chmod 0755 "$tmp/sing-box-tiny"

resolved=$(WLOC_SINGBOX_CANDIDATES="$tmp/sing-box-tiny" sh "$helper" path)
[ "$resolved" = "$tmp/sing-box-tiny" ]
version=$(WLOC_SINGBOX_CANDIDATES="$tmp/sing-box-tiny" sh "$helper" version)
printf '%s\n' "$version" | grep -F '1.12.0-tiny' >/dev/null

cat > "$tmp/not-sing-box" <<'EOF'
#!/bin/sh
exit 1
EOF
chmod 0755 "$tmp/not-sing-box"
if WLOC_SINGBOX_BIN="$tmp/not-sing-box" WLOC_SINGBOX_CANDIDATES="$tmp/sing-box-tiny" sh "$helper" path; then
	echo 'an explicitly selected invalid provider must not silently fall back' >&2
	exit 1
fi

echo 'sing-box runtime provider tests passed'
