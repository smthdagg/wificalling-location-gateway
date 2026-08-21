#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
budget=${WLOC_RESOURCE_BUDGET:-$repo_root/openwrt/files/usr/share/wificalling-location-gateway/resource-budget.conf}
package=${1:-}

fail() { echo "verify-package-budget: $*" >&2; exit 1; }
value() { sed -n "s/^$1=\([0-9][0-9]*\)$/\1/p" "$budget" | head -n 1; }

[ -n "$package" ] || fail 'usage: verify-package-budget.sh PACKAGE'
[ -f "$budget" ] && [ ! -L "$budget" ] || fail 'budget file must be a regular file'
[ "$(sed -n 's/^format=//p' "$budget")" = wfc-resource-budget/v1 ] || fail 'unsupported budget format'
[ -f "$package" ] && [ ! -L "$package" ] || fail 'package artifact must be a regular file'

size=$(wc -c < "$package" | tr -d ' ')
case "$size" in ''|*[!0-9]*) fail 'package size is not numeric' ;; esac
limit=$(value integrated_package_max_bytes)
[ -n "$limit" ] || fail 'integrated package limit is required'
[ "$size" -le "$limit" ] || fail "integrated package exceeds ${limit} bytes: $size"
printf 'package budget passed: bytes=%s\n' "$size"
