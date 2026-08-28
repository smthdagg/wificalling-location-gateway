#!/bin/sh
# Synchronize the node-probe files into a merged Gateway payload.
#
# Node tests must use the one Gateway sing-box instance. The maintained
# compiler adds loopback probe inbounds and the health/manual helpers call
# those inbounds; copying the complete set keeps an older merged payload from
# mixing incompatible compiler and probe formats.

set -eu

payload=${1:?payload directory required}
repo_root=$(CDPATH='' cd -- "$(dirname "$0")/../.." && pwd)

for relative in \
	etc/init.d/wificalling-gateway \
	usr/libexec/wificalling-gateway/compiler.sh \
	usr/libexec/wificalling-gateway/node-health.sh \
	usr/libexec/wificalling-gateway/node-test.sh; do
	src="$repo_root/openwrt/files/$relative"
	dst="$payload/$relative"
	[ -f "$src" ] || { echo "patch-wireguard-health: missing source $src" >&2; exit 2; }
	[ -f "$dst" ] || { echo "patch-wireguard-health: missing payload $dst" >&2; exit 2; }
	mkdir -p "$(dirname "$dst")"
	cp "$src" "$dst"
done

chmod 0755 \
	"$payload/etc/init.d/wificalling-gateway" \
	"$payload/usr/libexec/wificalling-gateway/compiler.sh" \
	"$payload/usr/libexec/wificalling-gateway/node-health.sh" \
	"$payload/usr/libexec/wificalling-gateway/node-test.sh"
printf '%s\n' 'patch-wireguard-health: synchronized single-process node probes' >&2
