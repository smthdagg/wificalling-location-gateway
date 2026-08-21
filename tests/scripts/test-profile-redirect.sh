#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
helper="$repo_root/openwrt/files/usr/sbin/wloc-profile-redirect.sh"
refresh="$repo_root/openwrt/files/usr/sbin/wloc-refresh-set.sh"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/wloc-profile-redirect.XXXXXX")
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

sh -n "$helper" "$refresh"
mkdir -p "$tmp/bin"
cat > "$tmp/bin/nft" <<'EOF'
#!/bin/sh
printf 'nft %s\n' "$*" >> "$WLOC_TEST_LOG"
if [ "$*" = 'list tables inet' ]; then
  printf '%s\n' 'table inet wloc_profile_phone' 'table inet wloc_profile_tablet'
fi
exit 0
EOF
cat > "$tmp/bin/uci" <<'EOF'
#!/bin/sh
printf '%s\n' '192.168.1.1'
EOF
cat > "$tmp/bin/nslookup" <<'EOF'
#!/bin/sh
printf '%s\n' 'Address: 59.82.17.33'
EOF
chmod 0755 "$tmp/bin/nft" "$tmp/bin/uci" "$tmp/bin/nslookup"
: > "$tmp/commands.log"

PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" \
  "$helper" start phone 192.168.1.10
PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" \
  "$helper" start tablet 192.168.1.11
PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" \
  "$helper" stop phone

grep -F 'nft add table inet wloc_profile_phone' "$tmp/commands.log" >/dev/null
grep -F 'nft add table inet wloc_profile_tablet' "$tmp/commands.log" >/dev/null
grep -F 'nft delete table inet wloc_profile_phone' "$tmp/commands.log" >/dev/null
if grep -F 'nft delete table inet wloc_profile_tablet' "$tmp/commands.log" >/dev/null; then
  printf '%s\n' 'stopping one profile must not delete another profile table' >&2
  exit 1
fi
grep -F 'ip saddr 192.168.1.10 tcp dport 443' "$tmp/commands.log" >/dev/null
grep -F 'ip daddr @apple_hosts' "$tmp/commands.log" >/dev/null

PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" "$refresh"
grep -F 'nft flush set inet wloc_profile_phone apple_hosts' "$tmp/commands.log" >/dev/null
grep -F 'nft flush set inet wloc_profile_tablet apple_hosts' "$tmp/commands.log" >/dev/null

if PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" \
  "$helper" start '../phone' 192.168.1.12; then
  printf '%s\n' 'path traversal profile id must be rejected' >&2
  exit 1
fi
if PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" \
  "$helper" start bad-mac aa:bb:cc:dd:ee:ff; then
  printf '%s\n' 'MAC binding must remain unsupported by the IP-only runtime' >&2
  exit 1
fi

if grep -E 'udp[[:space:]]+500|udp[[:space:]]+4500|gs-loc-corpa|apple\.com\.cn|bluedot' \
  "$helper" >/dev/null; then
  printf '%s\n' 'profile redirect helper exceeded the approved WLOC scope' >&2
  exit 1
fi

printf '%s\n' 'profile redirect shell tests passed'
