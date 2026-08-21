#!/bin/sh
# Create the signed sidecar manifest consumed by wloc-component-update.sh.
# The package itself is never modified, so its SHA-256 remains stable.

set -eu

package=${1:-}
[ -n "$package" ] || { echo "usage: $0 PACKAGE.ipk" >&2; exit 2; }
[ -f "$package" ] && [ ! -L "$package" ] || { echo 'package must be a regular file' >&2; exit 2; }

tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-update-manifest.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

tar -xOf "$package" control.tar.gz > "$tmp/control.tar.gz" 2>/dev/null \
	|| tar -xOf "$package" ./control.tar.gz > "$tmp/control.tar.gz"
tar -xOf "$package" data.tar.gz > "$tmp/data.tar.gz" 2>/dev/null \
	|| tar -xOf "$package" ./data.tar.gz > "$tmp/data.tar.gz"
control=$(tar -xOf "$tmp/control.tar.gz" ./control)
name=$(printf '%s\n' "$control" | sed -n 's/^Package:[[:space:]]*//p' | head -n 1)
version=$(printf '%s\n' "$control" | sed -n 's/^Version:[[:space:]]*//p' | head -n 1)
architecture=$(printf '%s\n' "$control" | sed -n 's/^Architecture:[[:space:]]*//p' | head -n 1)
sha256_file() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1" | awk '{print $1}'
	else
		shasum -a 256 "$1" | awk '{print $1}'
	fi
}

manifest="$package.manifest"
{
	printf '%s\n' 'Format: wfc-update-manifest/v1'
	printf 'Package: %s\nVersion: %s\nArchitecture: %s\n' "$name" "$version" "$architecture"
	printf 'Package-SHA256: %s\n' "$(sha256_file "$package")"
	printf 'Control-SHA256: %s\n' "$(sha256_file "$tmp/control.tar.gz")"
	printf 'Data-SHA256: %s\n' "$(sha256_file "$tmp/data.tar.gz")"
} > "$manifest"
chmod 0600 "$manifest"

if [ -n "${WFC_UPDATE_SIGNING_KEY:-}" ]; then
	usign=${WFC_UPDATE_USIGN:-/usr/bin/usign}
	[ -x "$usign" ] || { echo 'configured WFC_UPDATE_SIGNING_KEY requires usign' >&2; exit 2; }
	"$usign" -S -m "$manifest" -s "$WFC_UPDATE_SIGNING_KEY" -x "$package.sig"
	chmod 0600 "$package.sig"
fi

printf '%s\n' "$manifest"
