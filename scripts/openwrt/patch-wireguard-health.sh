#!/bin/sh
# Patch the Gateway node-health.sh so WireGuard nodes are validated by a
# real handshake instead of ICMP ping alone.
#
# The stock checker only ICMP-pings the server (with a TCP fallback that
# does not cover wireguard), so an imported wireguard node can show
# "reachable" while its handshake never completes (missing/mismatched
# preshared key, wrong endpoint, ...). This patch adds a handshake test
# that brings up a temporary sing-box wireguard endpoint, asks an echo
# service through it, and reports handshake_ok/handshake_failed with the
# exit IP. Results are cached for 60s because the monitor loop runs every
# 5s and a handshake test takes seconds.
#
# Fail-closed: any missing target string aborts the build. Idempotent.

set -eu

payload=${1:?payload directory required}
health="$payload/usr/libexec/wificalling-gateway/node-health.sh"

[ -f "$health" ] || { echo "patch-wireguard-health: missing $health" >&2; exit 2; }

python3 - "$health" <<'PY'
import sys

health = sys.argv[1]

with open(health, encoding='utf-8') as handle:
    text = handle.read()

if 'wg_handshake_test' in text:
    print('patch-wireguard-health: already patched', file=sys.stderr)
    raise SystemExit(0)

edits = [
    # 1. handshake test function after json_escape
    (
        "json_escape() {\n\tprintf '%s' \"$1\" | sed 's/\\\\/\\\\\\\\/g; s/\"/\\\\\"/g'\n}\n",
        "json_escape() {\n\tprintf '%s' \"$1\" | sed 's/\\\\/\\\\\\\\/g; s/\"/\\\\\"/g'\n}\n\n"
        "# Real WireGuard handshake validation: run a temporary sing-box\n"
        "# endpoint for the node and ask an echo service through it. The\n"
        "# result is cached for 60s (the monitor loop runs every 5s and a\n"
        "# handshake test takes seconds). Prints the exit IP on success.\n"
        "# The reserved field is forwarded too: WARP-style endpoints need it\n"
        "# and would otherwise fail every handshake. Cache line 3 carries the\n"
        "# failure reason (config_missing / timeout / unreachable) so the\n"
        "# status export can tell a bad node apart from a dead server.\n"
        "# A mkdir lock serializes the actual tests: the monitor loop can\n"
        "# tick a fresh instance before this one finished (a handshake takes\n"
        "# up to ~8s, the loop ticks every 5s), and two instances racing on\n"
        "# the same probe port would hand each other the wrong exit IP.\n"
        "wg_handshake_test() {\n"
        "\tid=$1; server=$2; port=$3\n"
        "\tcache=\"/tmp/wg-health-$id\"\n"
        "\tlock=/tmp/wg-health.lock\n"
        "\tif [ -f \"$cache\" ]; then\n"
        "\t\tcache_ts=$(sed -n '1p' \"$cache\" 2>/dev/null || echo 0)\n"
        "\t\tage=$(($(date +%s) - ${cache_ts:-0}))\n"
        "\t\tif [ \"$age\" -lt 60 ] 2>/dev/null; then\n"
        "\t\t\t[ \"$(sed -n '2p' \"$cache\")\" = ok ] || return 1\n"
        "\t\t\tsed -n '3p' \"$cache\"\n"
        "\t\t\treturn 0\n"
        "\t\tfi\n"
        "\tfi\n"
        "\tif ! mkdir \"$lock\" 2>/dev/null; then\n"
        "\t\t# A tick killed mid-test (SIGHUP/reboot) can leave the lock\n"
        "\t\t# behind; its holder PID is gone, so take it over.\n"
        "\t\tlock_pid=$(cat \"$lock/pid\" 2>/dev/null || echo 0)\n"
        "\t\tif ! kill -0 \"$lock_pid\" 2>/dev/null; then\n"
        "\t\t\trm -f \"$lock/pid\"; rmdir \"$lock\" 2>/dev/null || true\n"
        "\t\t\tmkdir \"$lock\" 2>/dev/null || true\n"
        "\t\tfi\n"
        "\tfi\n"
        "\tif ! [ -d \"$lock\" ]; then\n"
        "\t\t# Another monitor tick is testing right now; use the cache\n"
        "\t\t# as-is (even stale) instead of racing on the probe port.\n"
        "\t\tif [ -f \"$cache\" ] && [ \"$(sed -n '2p' \"$cache\")\" = ok ]; then\n"
        "\t\t\tsed -n '3p' \"$cache\"\n"
        "\t\t\treturn 0\n"
        "\t\tfi\n"
        "\t\treturn 1\n"
        "\tfi\n"
        "\techo $$ > \"$lock/pid\"\n"
        "\tpriv=$(uci -q get \"wificalling-gateway.$id.private_key\") || true\n"
        "\tpub=$(uci -q get \"wificalling-gateway.$id.public_key\") || true\n"
        "\tlocal_addr=$(uci -q get \"wificalling-gateway.$id.local_address\") || true\n"
        "\tif [ -z \"$priv\" ] || [ -z \"$pub\" ] || [ -z \"$local_addr\" ]; then\n"
        "\t\tprintf '%s\\nfailed\\nconfig_missing\\n' \"$(date +%s)\" > \"$cache\"\n"
        "\t\trm -f \"$lock/pid\"; rmdir \"$lock\" 2>/dev/null || true\n"
        "\t\treturn 1\n"
        "\tfi\n"
        "\tpsk=$(uci -q get \"wificalling-gateway.$id.pre_shared_key\") || true\n"
        "\tmtu=$(uci -q get \"wificalling-gateway.$id.mtu\") || true\n"
        "\treserved=$(uci -q get \"wificalling-gateway.$id.reserved\") || true\n"
        "\tlport=$((19000 + (0x$(printf '%s' \"$id\" | md5sum | cut -c1-4) % 1000))) 2>/dev/null || lport=19099\n"
        "\tcfg=\"/tmp/wg-health-$id.json\"\n"
        "\t{\n"
        "\t\tprintf '{\"log\":{\"level\":\"debug\"},\"inbounds\":[{\"type\":\"http\",\"tag\":\"probe\",\"listen\":\"127.0.0.1\",\"listen_port\":%s}],' \"$lport\"\n"
        "\t\tprintf '\"endpoints\":[{\"type\":\"wireguard\",\"tag\":\"wg\",\"address\":[%s],\"private_key\":%s,\"peers\":[{\"address\":%s,\"port\":%s,\"public_key\":%s,\"allowed_ips\":[\"0.0.0.0/0\"]' \\\n"
        "\t\t\t\"\\\"$local_addr\\\"\" \"\\\"$priv\\\"\" \"\\\"$server\\\"\" \"$port\" \"\\\"$pub\\\"\"\n"
        "\t\t[ -n \"$psk\" ] && printf ',\"pre_shared_key\":\"%s\"' \"$psk\"\n"
        "\t\t[ -n \"$reserved\" ] && printf ',\"reserved\":[%s]' \"$(printf '%s' \"$reserved\" | tr -d ' ')\"\n"
        "\t\tprintf '}],\"mtu\":%s}],\"outbounds\":[{\"type\":\"direct\",\"tag\":\"direct\"}],\"route\":{\"final\":\"wg\"}}' \"${mtu:-1420}\"\n"
        "\t} > \"$cfg\"\n"
        "\t/usr/bin/sing-box run -c \"$cfg\" > /tmp/wg-health-$id.log 2>&1 &\n"
        "\tpid=$!\n"
        "\tsleep 2\n"
        "\tip=$(curl -s --max-time 6 -x \"http://127.0.0.1:$lport\" 'http://ip-api.com/json?fields=query' 2>/dev/null | sed -n 's/.*\"query\":\"\\([0-9.]*\\)\".*/\\1/p' || true)\n"
        "\tkill \"$pid\" 2>/dev/null || true\n"
        "\twait \"$pid\" 2>/dev/null || true\n"
        "\tif [ -n \"$ip\" ]; then\n"
        "\t\trm -f \"$cfg\" /tmp/wg-health-$id.log\n"
        "\t\tprintf '%s\\nok\\n%s\\n' \"$(date +%s)\" \"$ip\" > \"$cache\"\n"
        "\t\trm -f \"$lock/pid\"; rmdir \"$lock\" 2>/dev/null || true\n"
        "\t\tprintf '%s' \"$ip\"\n"
        "\t\treturn 0\n"
        "\tfi\n"
        "\tif grep -q 'handshake did not complete' /tmp/wg-health-$id.log 2>/dev/null; then\n"
        "\t\treason=timeout\n"
        "\telse\n"
        "\t\treason=unreachable\n"
        "\tfi\n"
        "\trm -f \"$cfg\" /tmp/wg-health-$id.log\n"
        "\tprintf '%s\\nfailed\\n%s\\n' \"$(date +%s)\" \"$reason\" > \"$cache\"\n"
        "\trm -f \"$lock/pid\"; rmdir \"$lock\" 2>/dev/null || true\n"
        "\treturn 1\n"
        "}\n",
    ),
    # 2. wireguard override after the ICMP/TCP branch
    (
        "\t\t[ \"$first\" -eq 1 ] || printf ','\n",
        "\t\t# WireGuard nodes are validated by a real handshake, not ICMP.\n"
        "\t\tif [ \"$protocol\" = wireguard ]; then\n"
        "\t\t\tmeasurement=wg_handshake\n"
        "\t\t\tif exit_ip=$(wg_handshake_test \"$id\" \"$server\" \"$port\"); then\n"
        "\t\t\t\tstate=handshake_ok; ping_json=$exit_ip; reason_json=null\n"
        "\t\t\telse\n"
        "\t\t\t\tstate=handshake_failed; ping_json=null\n"
        "\t\t\t\treason_json=\"\\\"$(sed -n '3p' \"/tmp/wg-health-$id\" 2>/dev/null || echo unreachable)\\\"\"\n"
        "\t\t\tfi\n"
        "\t\telse\n"
        "\t\t\treason_json=null\n"
        "\t\tfi\n"
        "\t\t[ \"$first\" -eq 1 ] || printf ','\n",
    ),
]

for old, new in edits:
    if old not in text:
        print(
            f'patch-wireguard-health: target not found; '
            'gateway version mismatch?',
            file=sys.stderr,
        )
        raise SystemExit(2)
    text = text.replace(old, new, 1)

with open(health, 'w', encoding='utf-8') as handle:
    handle.write(text)

print('patch-wireguard-health: applied handshake validation', file=sys.stderr)
PY
