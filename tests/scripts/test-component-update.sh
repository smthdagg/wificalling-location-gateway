#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-update-test.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
root="$tmp/root"
state="$tmp/state"
mkdir -p "$root/etc/config" "$root/usr/share/wificalling-location-gateway" "$state" "$tmp/bin"
printf 'old-config\n' > "$root/etc/config/wloc-service"
printf 'old-component\n' > "$root/usr/share/wificalling-location-gateway/component.txt"
cat > "$root/etc/openwrt_release" <<'EOF'
DISTRIB_RELEASE='24.10.5'
DISTRIB_TARGET='mediatek/mt7622'
DISTRIB_ARCH='aarch64_cortex-a53'
EOF
printf '1.0.0-1\n' > "$state/current.version"

cat > "$tmp/bin/opkg" <<'EOF'
#!/bin/sh
set -eu
if [ "${1:-}" = print-architecture ]; then
    printf '%s\n' 'arch all 1' 'arch noarch 1' 'arch aarch64_cortex-a53 10'
    exit 0
fi
force=0
while [ "${1:-}" != install ] && [ "$#" -gt 0 ]; do
    [ "$1" = --force-downgrade ] && force=1
    shift
done
[ "${1:-}" = install ] || exit 2
shift
if [ "${WLOC_UPDATE_FAIL_ROLLBACK:-0}" = 1 ] && [ "$force" = 1 ]; then
    exit 1
fi
printf '%s\n' install >> "$WLOC_UPDATE_OPKG_LOG"
package=$1
data="$WLOC_UPDATE_TEST_TMP/data.tar.gz"
tar -xOf "$package" ./data.tar.gz > "$data"
tar -xzf "$data" -C "$WLOC_UPDATE_ROOT"
EOF
cat > "$tmp/bin/supervisor" <<'EOF'
#!/bin/sh
printf '%s\n' restart >> "$WLOC_UPDATE_SUPERVISOR_LOG"
EOF
cat > "$tmp/bin/health" <<'EOF'
#!/bin/sh
[ "$(cat "$WLOC_UPDATE_HEALTH_STATE")" = fail-always ] && exit 1
[ "$(cat "$WLOC_UPDATE_HEALTH_STATE")" = fail-once ] && { printf 'ok\n' > "$WLOC_UPDATE_HEALTH_STATE"; exit 1; }
[ "$(cat "$WLOC_UPDATE_HEALTH_STATE")" = ok ] || exit 1
printf '%s\n' '{"services":{"wloc":{"running":1,"socket":1,"status_fresh":1},"provider":{"available":1,"valid":1,"config_present":1,"config_valid":1},"redirect":{"table_present":1,"rules":1}}}'
EOF
chmod 0755 "$tmp/bin/opkg" "$tmp/bin/supervisor" "$tmp/bin/health"
cat > "$tmp/bin/usign" <<'EOF'
#!/bin/sh
exit 0
EOF
chmod 0755 "$tmp/bin/usign"

make_ipk() {
    version=$1
    component=$2
    out=$3
    architecture=${4:-all}
    target=${5:-mediatek/mt7622}
    package_dir="$tmp/package-$version-$component"
    rm -rf "$package_dir"
    mkdir -p "$package_dir/control" "$package_dir/data/etc/config" "$package_dir/data/usr/share/wificalling-location-gateway"
    cat > "$package_dir/control/control" <<EOF
Package: wificalling-location-gateway
Version: $version
Architecture: $architecture
X-WLOC-Product: wificalling-location-gateway/v2
X-WLOC-Api: wloc.service/v2
X-WLOC-OpenWrt: 24.10+
X-WLOC-Target: $target
X-WLOC-Package-Format: ipk
EOF
    printf '%s\n' "$component" > "$package_dir/data/usr/share/wificalling-location-gateway/component.txt"
    printf 'new-config\n' > "$package_dir/data/etc/config/wloc-service"
    (cd "$package_dir/control" && tar -czf "$package_dir/control.tar.gz" ./control)
    (cd "$package_dir/data" && tar -czf "$package_dir/data.tar.gz" ./etc ./usr)
    (cd "$package_dir" && tar -czf "$out" ./control.tar.gz ./data.tar.gz)
}

make_manifest() {
    package=$1
    control="$tmp/manifest-control.tar.gz"
    data="$tmp/manifest-data.tar.gz"
    tar -xOf "$package" ./control.tar.gz > "$control"
    tar -xOf "$package" ./data.tar.gz > "$data"
    {
        printf '%s\n' 'Format: wloc-update-manifest/v1'
        tar -xOf "$control" ./control | sed -n -e '/^Package:/p' -e '/^Version:/p' -e '/^Architecture:/p'
        printf 'Package-SHA256: %s\n' "$(sha256sum "$package" | awk '{print $1}')"
        printf 'Control-SHA256: %s\n' "$(sha256sum "$control" | awk '{print $1}')"
        printf 'Data-SHA256: %s\n' "$(sha256sum "$data" | awk '{print $1}')"
    } > "$package.manifest"
    : > "$package.sig"
}

old_package="$tmp/old.ipk"
new_package="$tmp/new.ipk"
bad_arch_package="$tmp/bad-arch.ipk"
bad_target_package="$tmp/bad-target.ipk"
make_ipk 1.0.0-1 old-component "$old_package"
make_ipk 1.1.0-1 new-component "$new_package"
make_ipk 1.1.0-1 bad-component "$bad_arch_package" mipsel
make_ipk 1.1.0-1 bad-target "$bad_target_package" all x86/64
make_manifest "$old_package"
make_manifest "$new_package"
make_manifest "$bad_arch_package"
make_manifest "$bad_target_package"

# Rebuilding an unsigned manifest must not leave a stale detached signature
# beside it; otherwise a later update could verify a signature for old content.
stale_package="$tmp/stale.ipk"
cp "$new_package" "$stale_package"
printf '%s\n' 'stale-signature' > "$stale_package.sig"
WLOC_UPDATE_SIGNING_KEY= WLOC_UPDATE_USIGN= \
    "$repo_root/scripts/create-update-manifest.sh" "$stale_package" >/dev/null
[ ! -e "$stale_package.sig" ]
[ -s "$stale_package.manifest" ]

export WLOC_UPDATE_TEST_TMP="$tmp"
export WLOC_UPDATE_ROOT="$root"
export WLOC_UPDATE_STATE_DIR="$state"
export WLOC_UPDATE_OPKG="$tmp/bin/opkg"
export WLOC_UPDATE_OPKG_LOG="$tmp/opkg.log"
export WLOC_UPDATE_SUPERVISOR="$tmp/bin/supervisor"
export WLOC_UPDATE_SUPERVISOR_LOG="$tmp/supervisor.log"
export WLOC_UPDATE_HEALTH="$tmp/bin/health"
export WLOC_UPDATE_HEALTH_TIMEOUT=1
export WLOC_UPDATE_HEALTH_STATE="$tmp/health"
export WLOC_UPDATE_FREE_KB=65536
export WLOC_UPDATE_ALLOW_ANY_SOURCE=1
export WLOC_UPDATE_USIGN="$tmp/bin/usign"
export WLOC_UPDATE_PUBLIC_KEY="$tmp/update.pub"
export TMPDIR="$tmp"
printf 'test-public-key\n' > "$tmp/update.pub"
printf 'ok\n' > "$tmp/health"
: > "$tmp/opkg.log"
: > "$tmp/supervisor.log"

script="$repo_root/openwrt/files/usr/sbin/wloc-component-update.sh"

cp "$new_package" "$tmp/unsigned.ipk"
if sh "$script" preflight "$tmp/unsigned.ipk"; then
    echo 'unsigned update package was accepted' >&2
    exit 1
fi
cp "$new_package.manifest" "$tmp/tampered.manifest"
sed 's/^Package-SHA256:.*/Package-SHA256: 0000000000000000000000000000000000000000000000000000000000000000/' \
    "$tmp/tampered.manifest" > "$tmp/tampered.manifest.new"
mv "$tmp/tampered.manifest.new" "$tmp/tampered.manifest"
if WLOC_UPDATE_MANIFEST="$tmp/tampered.manifest" sh "$script" preflight "$new_package"; then
    echo 'tampered update manifest was accepted' >&2
    exit 1
fi

WLOC_UPDATE_CURRENT_PACKAGE="$old_package" sh "$script" apply "$new_package"
grep '^install$' "$tmp/opkg.log" >/dev/null
grep '^new-component$' "$root/usr/share/wificalling-location-gateway/component.txt" >/dev/null
grep '^old-config$' "$root/etc/config/wloc-service" >/dev/null
grep '^1.1.0-1$' "$state/current.version" >/dev/null

if sh "$script" apply "$bad_arch_package"; then
    echo 'incompatible architecture was accepted' >&2
    exit 1
fi
if sh "$script" apply "$bad_target_package"; then
    echo 'incompatible firmware target was accepted' >&2
    exit 1
fi
if find "$tmp" -maxdepth 1 -type d -name 'wloc-update-check.*' -print -quit | grep -q .; then
    echo 'failed package validation leaked its temporary work directory' >&2
    exit 1
fi
if sh "$script" apply "$old_package"; then
    echo 'unauthorized downgrade was accepted' >&2
    exit 1
fi
WLOC_UPDATE_ALLOW_DOWNGRADE=1 WLOC_UPDATE_CURRENT_PACKAGE="$new_package" \
    sh "$script" apply "$old_package"
grep '^old-component$' "$root/usr/share/wificalling-location-gateway/component.txt" >/dev/null
WLOC_UPDATE_CURRENT_PACKAGE="$old_package" sh "$script" apply "$new_package"
grep '^new-component$' "$root/usr/share/wificalling-location-gateway/component.txt" >/dev/null

printf 'fail-always\n' > "$tmp/health"
if WLOC_UPDATE_CURRENT_PACKAGE="$new_package" sh "$script" apply "$new_package"; then
    echo 'health failure was accepted' >&2
    exit 1
fi
grep '^new-component$' "$root/usr/share/wificalling-location-gateway/component.txt" >/dev/null
grep '^old-config$' "$root/etc/config/wloc-service" >/dev/null
grep 'rollback_failed' "$state/status.json" >/dev/null
printf 'ok\n' > "$tmp/health"
WLOC_UPDATE_CURRENT_PACKAGE="$new_package" sh "$script" recover
grep 'rolled_back' "$state/status.json" >/dev/null

printf 'fail-always\n' > "$tmp/health"
if WLOC_UPDATE_FAIL_ROLLBACK=1 WLOC_UPDATE_CURRENT_PACKAGE="$new_package" \
    sh "$script" apply "$new_package"; then
    echo 'rollback failure was accepted' >&2
    exit 1
fi
grep 'rollback_failed' "$state/status.json" >/dev/null
[ -d "$state/transaction" ]
printf 'ok\n' > "$tmp/health"
WLOC_UPDATE_CURRENT_PACKAGE="$new_package" sh "$script" recover
grep 'rolled_back' "$state/status.json" >/dev/null
[ ! -e "$state/transaction" ]

printf 'ok\n' > "$tmp/health"
WLOC_UPDATE_INTERRUPT_AFTER_INSTALL=1 WLOC_UPDATE_CURRENT_PACKAGE="$new_package" \
    sh "$script" apply "$new_package" || true
mkdir "$state/.lock"
printf '999999\n' > "$state/.lock/pid"
sh "$script" recover
grep '^new-component$' "$root/usr/share/wificalling-location-gateway/component.txt" >/dev/null

fresh_state="$tmp/fresh-state"
WLOC_UPDATE_STATE_DIR="$fresh_state" sh "$script" preflight "$new_package" >/dev/null
[ -d "$fresh_state" ]

before=$(wc -l < "$tmp/opkg.log" | tr -d ' ')
if WLOC_UPDATE_FREE_KB=1 sh "$script" apply "$new_package"; then
    echo 'low storage was accepted' >&2
    exit 1
fi
after=$(wc -l < "$tmp/opkg.log" | tr -d ' ')
[ "$before" = "$after" ]
if find "$tmp" -maxdepth 1 -type d -name 'wloc-update-check.*' -print -quit | grep -q .; then
    echo 'low-space validation leaked its temporary work directory' >&2
    exit 1
fi

echo 'component update tests passed'
