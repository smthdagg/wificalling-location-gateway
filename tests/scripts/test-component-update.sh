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
printf '1.0.0-1\n' > "$state/current.version"

cat > "$tmp/bin/opkg" <<'EOF'
#!/bin/sh
set -eu
if [ "${1:-}" != install ]; then
    exit 2
fi
printf '%s\n' install >> "$WLOC_UPDATE_OPKG_LOG"
package=$2
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
[ "$(cat "$WLOC_UPDATE_HEALTH_STATE")" = ok ]
EOF
chmod 0755 "$tmp/bin/opkg" "$tmp/bin/supervisor" "$tmp/bin/health"

make_ipk() {
    version=$1
    component=$2
    out=$3
    package_dir="$tmp/package-$version-$component"
    rm -rf "$package_dir"
    mkdir -p "$package_dir/control" "$package_dir/data/etc/config" "$package_dir/data/usr/share/wificalling-location-gateway"
    cat > "$package_dir/control/control" <<EOF
Package: wificalling-location-gateway
Version: $version
Architecture: all
X-WFC-Product: wificalling-location-gateway/v2
X-WFC-Gateway: 1.7
X-WFC-Wloc-Api: wloc.service/v2
EOF
    printf '%s\n' "$component" > "$package_dir/data/usr/share/wificalling-location-gateway/component.txt"
    printf 'new-config\n' > "$package_dir/data/etc/config/wloc-service"
    (cd "$package_dir/control" && tar -czf "$package_dir/control.tar.gz" ./control)
    (cd "$package_dir/data" && tar -czf "$package_dir/data.tar.gz" ./etc ./usr)
    (cd "$package_dir" && tar -czf "$out" ./control.tar.gz ./data.tar.gz)
}

old_package="$tmp/old.ipk"
new_package="$tmp/new.ipk"
make_ipk 1.0.0-1 old-component "$old_package"
make_ipk 1.1.0-1 new-component "$new_package"

export WLOC_UPDATE_TEST_TMP="$tmp"
export WLOC_UPDATE_ROOT="$root"
export WLOC_UPDATE_STATE_DIR="$state"
export WLOC_UPDATE_OPKG="$tmp/bin/opkg"
export WLOC_UPDATE_OPKG_LOG="$tmp/opkg.log"
export WLOC_UPDATE_SUPERVISOR="$tmp/bin/supervisor"
export WLOC_UPDATE_SUPERVISOR_LOG="$tmp/supervisor.log"
export WLOC_UPDATE_HEALTH="$tmp/bin/health"
export WLOC_UPDATE_HEALTH_STATE="$tmp/health"
export WLOC_UPDATE_FREE_KB=65536
export WLOC_UPDATE_ALLOW_ANY_SOURCE=1
printf 'ok\n' > "$tmp/health"
: > "$tmp/opkg.log"
: > "$tmp/supervisor.log"

script="$repo_root/openwrt/files/usr/sbin/wloc-component-update.sh"

WLOC_UPDATE_CURRENT_PACKAGE="$old_package" sh "$script" apply "$new_package"
grep '^install$' "$tmp/opkg.log" >/dev/null
grep '^new-component$' "$root/usr/share/wificalling-location-gateway/component.txt" >/dev/null
grep '^old-config$' "$root/etc/config/wloc-service" >/dev/null
grep '^1.1.0-1$' "$state/current.version" >/dev/null

printf 'fail\n' > "$tmp/health"
if WLOC_UPDATE_CURRENT_PACKAGE="$new_package" sh "$script" apply "$new_package"; then
    echo 'health failure was accepted' >&2
    exit 1
fi
grep '^new-component$' "$root/usr/share/wificalling-location-gateway/component.txt" >/dev/null
grep '^old-config$' "$root/etc/config/wloc-service" >/dev/null
grep 'rolled_back' "$state/status.json" >/dev/null

printf 'ok\n' > "$tmp/health"
WLOC_UPDATE_INTERRUPT_AFTER_INSTALL=1 WLOC_UPDATE_CURRENT_PACKAGE="$new_package" \
    sh "$script" apply "$new_package" || true
sh "$script" recover
grep '^new-component$' "$root/usr/share/wificalling-location-gateway/component.txt" >/dev/null

before=$(wc -l < "$tmp/opkg.log" | tr -d ' ')
if WLOC_UPDATE_FREE_KB=1 sh "$script" apply "$new_package"; then
    echo 'low storage was accepted' >&2
    exit 1
fi
after=$(wc -l < "$tmp/opkg.log" | tr -d ' ')
[ "$before" = "$after" ]

echo 'component update tests passed'
