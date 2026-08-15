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
        "wg_handshake_test() {\n"
        "\tid=$1; server=$2; port=$3\n"
        "\tcache=\"/tmp/wg-health-$id\"\n"
        "\tif [ -f \"$cache\" ]; then\n"
        "\t\tcache_ts=$(sed -n '1p' \"$cache\" 2>/dev/null || echo 0)\n"
        "\t\tage=$(($(date +%s) - ${cache_ts:-0}))\n"
        "\t\tif [ \"$age\" -lt 60 ] 2>/dev/null; then\n"
        "\t\t\t[ \"$(sed -n '2p' \"$cache\")\" = ok ] || return 1\n"
        "\t\t\tsed -n '3p' \"$cache\"\n"
        "\t\t\treturn 0\n"
        "\t\tfi\n"
        "\tfi\n"
        "\tpriv=$(uci -q get \"wificalling-gateway.$id.private_key\") || return 1\n"
        "\tpub=$(uci -q get \"wificalling-gateway.$id.public_key\") || return 1\n"
        "\tlocal_addr=$(uci -q get \"wificalling-gateway.$id.local_address\") || return 1\n"
        "\tpsk=$(uci -q get \"wificalling-gateway.$id.pre_shared_key\") || true\n"
        "\tmtu=$(uci -q get \"wificalling-gateway.$id.mtu\") || true\n"
        "\tlport=$((19000 + (${id#cfg} % 1000))) 2>/dev/null || lport=19099\n"
        "\tcfg=\"/tmp/wg-health-$id.json\"\n"
        "\t{\n"
        "\t\tprintf '{\"log\":{\"level\":\"warn\"},\"inbounds\":[{\"type\":\"http\",\"tag\":\"probe\",\"listen\":\"127.0.0.1\",\"listen_port\":%s}],' \"$lport\"\n"
        "\t\tprintf '\"endpoints\":[{\"type\":\"wireguard\",\"tag\":\"wg\",\"address\":[%s],\"private_key\":%s,\"peers\":[{\"address\":%s,\"port\":%s,\"public_key\":%s,\"allowed_ips\":[\"0.0.0.0/0\"]' \\\n"
        "\t\t\t\"\\\"$local_addr\\\"\" \"\\\"$priv\\\"\" \"\\\"$server\\\"\" \"$port\" \"\\\"$pub\\\"\"\n"
        "\t\t[ -n \"$psk\" ] && printf ',\"pre_shared_key\":\"%s\"' \"$psk\"\n"
        "\t\tprintf '}],\"mtu\":%s}],\"outbounds\":[{\"type\":\"direct\",\"tag\":\"direct\"}],\"route\":{\"final\":\"wg\"}}' \"${mtu:-1420}\"\n"
        "\t} > \"$cfg\"\n"
        "\t/usr/bin/sing-box run -c \"$cfg\" > /tmp/wg-health-$id.log 2>&1 &\n"
        "\tpid=$!\n"
        "\tsleep 2\n"
        "\tip=$(curl -s --max-time 6 -x \"http://127.0.0.1:$lport\" 'http://ip-api.com/json?fields=query' 2>/dev/null | sed -n 's/.*\"query\":\"\\([0-9.]*\\)\".*/\\1/p' || true)\n"
        "\tkill \"$pid\" 2>/dev/null || true\n"
        "\twait \"$pid\" 2>/dev/null || true\n"
        "\trm -f \"$cfg\" /tmp/wg-health-$id.log\n"
        "\tif [ -n \"$ip\" ]; then\n"
        "\t\tprintf '%s\\nok\\n%s\\n' \"$(date +%s)\" \"$ip\" > \"$cache\"\n"
        "\t\tprintf '%s' \"$ip\"\n"
        "\t\treturn 0\n"
        "\tfi\n"
        "\tprintf '%s\\nfailed\\n' \"$(date +%s)\" > \"$cache\"\n"
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
        "\t\t\t\tstate=handshake_ok; ping_json=$exit_ip\n"
        "\t\t\telse\n"
        "\t\t\t\tstate=handshake_failed; ping_json=null\n"
        "\t\t\tfi\n"
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
