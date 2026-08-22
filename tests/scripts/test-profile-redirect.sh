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
state="$WLOC_NFT_STATE"
mkdir -p "$(dirname "$state")"
if [ "${1:-}" = add ] && [ "${2:-}" = table ] && [ "${3:-}" = inet ]; then
  : > "$state.${4:-unknown}"
fi
if [ "${1:-}" = delete ] && [ "${2:-}" = table ] && [ "${3:-}" = inet ]; then
  rm -f "$state.${4:-unknown}"
fi
if [ "$*" = 'list tables inet' ]; then
  for table_state in "$state".wloc_profile_*; do
    [ -f "$table_state" ] || continue
    table=${table_state#"$state."}
    printf 'table inet %s\n' "$table"
  done
fi
if [ "${1:-}" = list ] && [ "${2:-}" = table ] && [ "${3:-}" = inet ] && [ ! -f "$state.${4:-unknown}" ]; then
  exit 1
fi
exit 0
EOF
cat > "$tmp/bin/ip" <<'EOF'
#!/bin/sh
printf 'ip %s\n' "$*" >> "$WLOC_TEST_LOG"
EOF
cat > "$tmp/bin/uci" <<'EOF'
#!/bin/sh
case "$2" in
  show)
    printf '%s\n' 'wloc-service.phone=device' 'wloc-service.tablet=device'
    ;;
  get)
    case "$3" in
      network.lan.ipaddr) printf '%s\n' '192.168.1.1' ;;
      wloc-service.phone.enabled) printf '%s\n' '1' ;;
      wloc-service.tablet.enabled) printf '%s\n' '0' ;;
    esac
    ;;
esac
EOF
cat > "$tmp/bin/nslookup" <<'EOF'
#!/bin/sh
printf '%s\n' 'Address: 59.82.17.33'
EOF
chmod 0755 "$tmp/bin/nft" "$tmp/bin/ip" "$tmp/bin/uci" "$tmp/bin/nslookup"
: > "$tmp/commands.log"
WLOC_NFT_STATE="$tmp/nft-state"
export WLOC_NFT_STATE
WLOC_UPSTREAM_IP_FILE="$tmp/apple-upstream-ip"
export WLOC_UPSTREAM_IP_FILE

PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" WLOC_NFT_STATE="$WLOC_NFT_STATE" \
  "$helper" start phone 192.168.1.10
PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" WLOC_NFT_STATE="$WLOC_NFT_STATE" \
  "$helper" start tablet 192.168.1.11
PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" WLOC_NFT_STATE="$WLOC_NFT_STATE" \
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
grep -F 'ip rule add fwmark 1 lookup 100' "$tmp/commands.log" >/dev/null
grep -F 'ip route add local 0.0.0.0/0 dev lo table 100' "$tmp/commands.log" >/dev/null

PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" WLOC_NFT_STATE="$WLOC_NFT_STATE" "$refresh"
grep -F 'nft delete table inet wloc_service' "$tmp/commands.log" >/dev/null
grep -F 'nft delete table inet wloc_profile_tablet' "$tmp/commands.log" >/dev/null
if grep -F 'nft flush set inet wloc_profile_tablet apple_hosts' "$tmp/commands.log" >/dev/null; then
  printf '%s\n' 'disabled profile tables must not be refreshed' >&2
  exit 1
fi
if grep -F 'nft flush set inet wloc_profile_phone apple_hosts' "$tmp/commands.log" >/dev/null; then
  printf '%s\n' 'stopped profile tables must not be refreshed' >&2
  exit 1
fi

# A live profile must receive nft timeout expressions as element attributes,
# not as separators between elements (the latter is rejected by nft 1.0.x).
PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" WLOC_NFT_STATE="$WLOC_NFT_STATE" \
  "$helper" start phone 192.168.1.10
: > "$tmp/commands.log"
PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" WLOC_NFT_STATE="$WLOC_NFT_STATE" "$refresh"
grep -F 'nft add element inet wloc_profile_phone apple_hosts { 59.82.17.33 timeout 30s }' "$tmp/commands.log" >/dev/null

: > "$tmp/commands.log"
PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" WLOC_NFT_STATE="$WLOC_NFT_STATE" \
  "$helper" start phone 192.168.1.10
PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" WLOC_NFT_STATE="$WLOC_NFT_STATE" \
  "$helper" start tablet 192.168.1.11
: > "$tmp/commands.log"
PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" WLOC_NFT_STATE="$WLOC_NFT_STATE" "$helper" stop-all
grep -F 'nft delete table inet wloc_profile_phone' "$tmp/commands.log" >/dev/null
grep -F 'nft delete table inet wloc_profile_tablet' "$tmp/commands.log" >/dev/null
grep -F 'ip rule del fwmark 1 lookup 100' "$tmp/commands.log" >/dev/null

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
if PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" \
  "$helper" start phone-2 192.168.1.12; then
  printf '%s\n' 'hyphenated profile id must be rejected' >&2
  exit 1
fi
if PATH="$tmp/bin:$PATH" WLOC_TEST_LOG="$tmp/commands.log" \
  "$helper" start documentation 192.0.2.1; then
  printf '%s\n' 'non-private 192/8 address must be rejected' >&2
  exit 1
fi

if grep -E 'udp[[:space:]]+500|udp[[:space:]]+4500|gs-loc-corpa|apple\.com\.cn|bluedot' \
  "$helper" >/dev/null; then
  printf '%s\n' 'profile redirect helper exceeded the approved WLOC scope' >&2
  exit 1
fi

printf '%s\n' 'profile redirect shell tests passed'
