#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
release_builder="$repo_root/scripts/openwrt/build-release-packages.sh"
ax6s_builder="$repo_root/scripts/build-luci-ipk.sh"
runtime_packager="$repo_root/scripts/openwrt/package-singbox-lite.sh"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wlg-variant-test.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

fail() {
	printf 'FAIL: %s\n' "$*" >&2
	exit 1
}

[ -x "$runtime_packager" ] || fail 'missing sing-box Lite runtime packager'

printf '#!/bin/sh\nexit 0\n' > "$tmp/wloc-service"
printf '#!/bin/sh\nexit 0\n' > "$tmp/wloc-ctl"
printf 'mock x86_64 sing-box tiny\n' > "$tmp/sing-box-x86_64"
chmod 0755 "$tmp/wloc-service" "$tmp/wloc-ctl" "$tmp/sing-box-x86_64"
tiny_sha=$(shasum -a 256 "$tmp/sing-box-x86_64" | awk '{print $1}')

plan=$(
	"$release_builder" --plan \
		--variants standard,lite \
		--arch x86_64 \
		--service-bin "$tmp/wloc-service" \
		--ctl-bin "$tmp/wloc-ctl" \
		--singbox-lite-bin "$tmp/sing-box-x86_64" \
		--singbox-lite-sha256 "$tiny_sha"
)

for expected in \
	'wificalling-location-gateway_1.2.0-r1_x86_64.ipk' \
	'wificalling-location-gateway-lite_1.2.0-r1_x86_64.ipk' \
	'wificalling-location-gateway-1.2.0-r1.apk' \
	'wificalling-location-gateway-lite-1.2.0-r1.apk'; do
	printf '%s\n' "$plan" | grep -F "$expected" >/dev/null ||
		fail "dual-variant plan is missing $expected"
done

printf '%s\n' "$plan" | grep -F 'standard runtime: firmware /usr/bin/sing-box' >/dev/null ||
	fail 'standard plan must declare the firmware-owned sing-box runtime'
printf '%s\n' "$plan" | grep -F "lite runtime: $tiny_sha" >/dev/null ||
	fail 'lite plan must bind the supplied tiny runtime digest'

if "$release_builder" --plan --variants standard,lite --arch x86_64 \
	--service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl" \
	--singbox-lite-bin "$tmp/sing-box-x86_64" \
	--singbox-lite-sha256 0000000000000000000000000000000000000000000000000000000000000000 >"$tmp/out" 2>"$tmp/err"; then
	fail 'lite build plan must reject an unpinned tiny runtime'
fi
grep -F 'sing-box Lite SHA-256 mismatch' "$tmp/err" >/dev/null ||
	fail 'tiny digest rejection must be explicit'

if "$release_builder" --plan --variants lite --arch x86_64 \
	--service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl" \
	>"$tmp/out" 2>"$tmp/err"; then
	fail 'lite plan must require a tiny runtime input'
fi
grep -F -- '--singbox-lite-bin is required for the Lite variant' "$tmp/err" >/dev/null ||
	fail 'missing Lite runtime rejection must be explicit'

if "$release_builder" --plan --variants standard,beta --arch x86_64 \
	--service-bin "$tmp/wloc-service" --ctl-bin "$tmp/wloc-ctl" \
	>"$tmp/out" 2>"$tmp/err"; then
	fail 'unknown package variants must be rejected'
fi
grep -F 'variants must be standard, lite, or standard,lite' "$tmp/err" >/dev/null ||
	fail 'variant rejection must list the supported choices'

grep -F "conflicts='wificalling-location-gateway-lite'" "$ax6s_builder" >/dev/null ||
	fail 'standard package must conflict with the Lite package'
grep -F 'output_package=wificalling-location-gateway-lite' "$ax6s_builder" >/dev/null ||
	fail 'AX6S builder must define the Lite package identity'
grep -F "provides='wificalling-location-gateway, sing-box" "$ax6s_builder" >/dev/null ||
	fail 'Lite must provide the integrated product and sing-box runtime'
grep -F "conflicts='wificalling-location-gateway, sing-box'" "$ax6s_builder" >/dev/null ||
	fail 'Lite must not coexist with the standard product or standard sing-box package'
grep -F 'GOMEMLIMIT=24MiB' "$repo_root/openwrt/files/etc/init.d/wificalling-gateway" >/dev/null ||
	fail 'Lite runtime profile must retain the AX6S-validated sing-box heap limit'
grep -F '/usr/bin/sing-box' "$repo_root/openwrt/files/etc/init.d/wificalling-gateway" >/dev/null ||
	fail 'both variants must keep the shared sing-box executable contract'
grep -F '/usr/share/wificalling-location-gateway/sing-box-lite.gz' "$runtime_packager" >/dev/null ||
	fail 'Lite must keep the compressed runtime on persistent storage'
grep -F '/tmp/sing-box-lite' "$runtime_packager" >/dev/null ||
	fail 'Lite must expand sing-box into tmpfs at runtime'

printf '%s\n' 'Standard/Lite package variant tests passed'
