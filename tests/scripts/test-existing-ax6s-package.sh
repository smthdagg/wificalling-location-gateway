#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
version="2.0.0-$(date +%s)-$$"
out="$repo_root/dist/luci-app-wificalling-location-gateway_${version}_all.ipk"
data=$(mktemp "${TMPDIR:-/tmp}/wloc-existing-package.XXXXXX")
trap 'rm -f "$out" "$out.manifest" "$out.sig" "$data"' EXIT HUP INT TERM

"$repo_root/scripts/build-luci-ipk.sh" "$version" ax6s-existing >/dev/null
[ -s "$out.manifest" ]
grep -E '^Package-SHA256: [0-9a-f]{64}$' "$out.manifest" >/dev/null
tar -xOf "$out" data.tar.gz > "$data" 2>/dev/null \
	|| tar -xOf "$out" ./data.tar.gz > "$data"
for member in \
	./usr/sbin/wloc-component-update.sh \
	./usr/sbin/wloc-health.sh \
	./usr/sbin/wloc-support-bundle.sh \
	./etc/init.d/wificalling-location-gateway \
	./usr/libexec/wificalling-location-gateway/unified-supervisor.sh; do
	tar -tzf "$data" | grep -Fx "$member" >/dev/null || {
		echo "existing AX6S package is missing $member" >&2
		exit 1
	}
done

echo 'existing AX6S package tests passed'
