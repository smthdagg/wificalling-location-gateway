#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/wfc-node-health-lock.XXXXXX")
node_id="node-lock-$$"
node_a="target-a-$$"
node_b="target-b-$$"
trap 'rm -rf "$work"; rm -f "/tmp/node-health-$node_id" "/tmp/node-health-$node_a" "/tmp/node-health-$node_b"' EXIT HUP INT TERM
export LC_ALL=C

mkdir -p "$work/bin"
printf '%s\n' '#!/bin/sh' 'exit 99' > "$work/bin/curl"
printf '%s\n' '#!/bin/sh' 'sleep 2' 'printf "64 bytes from 203.0.113.1: seq=0 ttl=64 time=10.000 ms\\nround-trip min/avg/max = 10.000/10.000/10.000 ms\\n"' > "$work/bin/ping"
chmod 755 "$work/bin/curl" "$work/bin/ping"
printf '%s\n' "$node_id|Node A|vless|example.test|443|20001" > "$work/nodes"

PATH="$work/bin:$PATH" sh "$repo_root/openwrt/files/usr/libexec/wificalling-gateway/node-health.sh" "$work/nodes" "$work/first.json" &
first_pid=$!
sleep 1
PATH="$work/bin:$PATH" sh "$repo_root/openwrt/files/usr/libexec/wificalling-gateway/node-health.sh" "$work/nodes" "$work/second.json"
[ ! -e "$work/second.json" ]
wait "$first_pid"
grep -F '"state":"reachable","measurement":"icmp"' "$work/first.json" >/dev/null

echo 'node-health lock behavior passed'

calls="$work/calls"
cat > "$work/bin/ping" <<'PING'
#!/bin/sh
printf 'x\n' >> "$NODE_HEALTH_CALLS"
printf '64 bytes from 203.0.113.9: seq=0 ttl=64 time=10.000 ms\nround-trip min/avg/max = 10.000/10.000/10.000 ms\n'
PING
chmod +x "$work/bin/ping"
printf '%s\n' \
	"$node_a|Node A|vless|one.example|443|20001" \
	"$node_b|Node B|vless|two.example|443|20002" > "$work/nodes"

# A background refresh must exercise only its selected node. The remaining
# rows keep their last cached measurement rather than producing a five-node
# connection burst on a small router.
NODE_HEALTH_CALLS="$calls" PATH="$work/bin:$PATH" \
	sh "$repo_root/openwrt/files/usr/libexec/wificalling-gateway/node-health.sh" \
	"$work/nodes" "$work/targeted.json" "$node_a"
[ "$(wc -l < "$calls")" -eq 1 ]
grep -F "\"id\":\"$node_a\",\"state\":\"reachable\",\"measurement\":\"icmp\"" "$work/targeted.json" >/dev/null
grep -F "\"id\":\"$node_b\",\"state\":\"unknown\"" "$work/targeted.json" >/dev/null
node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' "$work/targeted.json"

NODE_HEALTH_CALLS="$calls" PATH="$work/bin:$PATH" \
	sh "$repo_root/openwrt/files/usr/libexec/wificalling-gateway/node-health.sh" \
	"$work/nodes" "$work/targeted.json" "$node_b"
[ "$(wc -l < "$calls")" -eq 2 ]
grep -F "\"id\":\"$node_a\",\"state\":\"reachable\",\"measurement\":\"icmp\"" "$work/targeted.json" >/dev/null
grep -F "\"id\":\"$node_b\",\"state\":\"reachable\",\"measurement\":\"icmp\"" "$work/targeted.json" >/dev/null
node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' "$work/targeted.json"

echo 'node-health targeted refresh behavior passed'
