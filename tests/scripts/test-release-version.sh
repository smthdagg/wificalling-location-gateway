#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-release-version-test.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}

[ -f "$repo_root/VERSION" ] || fail 'missing canonical VERSION file'
[ "$(cat "$repo_root/VERSION")" = 1.4.0 ] || fail 'canonical version must be 1.4.0'
grep -Eq '^version = "1\.4\.0"$' "$repo_root/Cargo.toml" ||
	fail 'Cargo package version must be 1.4.0'
grep -F 'webpki-roots = "=1.0.9"' "$repo_root/Cargo.toml" >/dev/null ||
	fail 'version bumps must not rewrite pinned dependency versions'
grep -Fx 'PKG_VERSION:=1.4.0' "$repo_root/openwrt/Makefile" >/dev/null ||
	fail 'OpenWrt runtime version must be 1.4.0'
grep -Fx 'PKG_RELEASE:=17' "$repo_root/openwrt/Makefile" >/dev/null ||
	fail 'OpenWrt runtime release must be 17'
grep -Fx 'PKG_VERSION:=1.4.0' "$repo_root/openwrt/luci-app-wificalling-location-gateway/Makefile" >/dev/null ||
	fail 'LuCI package version must be 1.4.0'
grep -Fx 'PKG_RELEASE:=17' "$repo_root/openwrt/luci-app-wificalling-location-gateway/Makefile" >/dev/null ||
	fail 'LuCI package release must be 17'

printf '#!/bin/sh\nexit 0\n' > "$tmp/wloc-service"
printf '#!/bin/sh\nexit 0\n' > "$tmp/wloc-ctl"
chmod 0755 "$tmp/wloc-service" "$tmp/wloc-ctl"

# The release builder validates input ELF architecture; the stub binaries in
# this test are shell scripts, so answer the arch probe with x86-64.
mkdir -p "$tmp/mock-bin"
cat > "$tmp/mock-bin/file" <<'FILE'
#!/bin/sh
printf '%s: ELF 64-bit LSB pie executable, x86-64, version 1 (SYSV)\n' "$1"
FILE
chmod 0755 "$tmp/mock-bin/file"
PATH="$tmp/mock-bin:$PATH"
export PATH
plan=$(
	"$repo_root/scripts/openwrt/build-release-packages.sh" --plan \
		--arch x86_64 --service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl"
)
for expected in \
	'wificalling-location-gateway_1.4.0-r17_x86_64.ipk' \
	'wificalling-location-gateway-1.4.0-r17.apk'; do
	printf '%s\n' "$plan" | grep -F "$expected" >/dev/null ||
		fail "release plan is missing $expected"
done

grep -F 'version=${1:-1.4.0-r17}' "$repo_root/scripts/build-luci-ipk.sh" >/dev/null ||
	fail 'AX6S standalone builder default must be 1.4.0 release 16'

printf '%s\n' 'release version tests passed'
