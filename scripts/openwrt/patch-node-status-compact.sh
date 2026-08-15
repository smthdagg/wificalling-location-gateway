#!/bin/sh
# Compact the Gateway node-status.json and export it under the uhttpd
# docroot, so the LuCI status column works on routers with many nodes.
#
# On some firmwares (observed on ImmortalWrt 24.10) the /ubus JSON-RPC
# channel truncates larger replies mid-flight (chunked encoding bug), so
# the LuCI fs.read promise never yields valid JSON once node-status.json
# grows past ~1KB - the "Node status" / latency / quality columns then
# render as "-" even though node-health.sh writes fine data. Plain GETs of
# static files under /www are served with Content-Length and are never
# truncated, so the status file is exported there and the LuCI view reads
# it with fetch(). The view only consumes id/state/measurement/ping_ms, so
# the other fields (note/label/protocol/server/port) are dropped to keep
# the public copy minimal.
#
# Fail-closed: any missing target string aborts the build. Idempotent.

set -eu

payload=${1:?payload directory required}
health="$payload/usr/libexec/wificalling-gateway/node-health.sh"

[ -f "$health" ] || { echo "patch-node-status-compact: missing $health" >&2; exit 2; }

python3 - "$health" <<'PY'
import sys

health = sys.argv[1]

with open(health, encoding='utf-8') as handle:
    text = handle.read()

if 'compact_status_marker' in text:
    print('patch-node-status-compact: already patched', file=sys.stderr)
    raise SystemExit(0)

edits = [
    # Export under the uhttpd docroot: LuCI reads it with a plain GET,
    # because the /ubus JSON-RPC channel truncates larger replies on some
    # firmwares, leaving the node status blank.
    (
        'output=${2:-/var/run/wificalling-gateway/node-status.json}',
        '# compact_status_marker: static export for the LuCI view (the\n'
        '# /ubus JSON-RPC channel truncates larger replies on some firmwares).\n'
        'output=/www/wloc-node-status.json',
    ),
    # Drop the note field from the header line.
    (
        '\tprintf \'{"generated_at":%s,"note":"ICMP ping only; this is not a proxy protocol handshake.","nodes":[\' "$(date +%s)"',
        '\tprintf \'{"generated_at":%s,"nodes":[\' "$(date +%s)"',
    ),
    # Emit only the fields the LuCI view actually reads.
    (
        "\t\tprintf '{\"id\":\"%s\",\"label\":\"%s\",\"protocol\":\"%s\",\"server\":\"%s\",\"port\":%s,\"state\":\"%s\",\"measurement\":\"%s\",\"ping_ms\":%s}' \\\n"
        "\t\t\t\"$(json_escape \"$id\")\" \"$(json_escape \"$label\")\" \"$(json_escape \"$protocol\")\" \\\n"
        "\t\t\t\"$(json_escape \"$server\")\" \"$port\" \"$state\" \"$measurement\" \"$ping_json\"",
        "printf '{\"id\":\"%s\",\"state\":\"%s\",\"measurement\":\"%s\",\"ping_ms\":%s}' \\\n"
        "\t\t\t\"$(json_escape \"$id\")\" \"$state\" \"$measurement\" \"$ping_json\"",
    ),
    # The handshake exit IP is a string and must be quoted, otherwise the
    # whole status document is invalid JSON ("ping_ms":1.2.3.4) and the
    # LuCI view renders every status column as "-".
    (
        "\t\t\t\tstate=handshake_ok; ping_json=$exit_ip",
        "\t\t\t\tstate=handshake_ok; ping_json=\"\\\"$exit_ip\\\"\"",
    ),
]

for old, new in edits:
    if old not in text:
        print(
            f'patch-node-status-compact: target not found; '
            'gateway version mismatch?',
            file=sys.stderr,
        )
        raise SystemExit(2)
    text = text.replace(old, new, 1)

with open(health, 'w', encoding='utf-8') as handle:
    handle.write(text)

print('patch-node-status-compact: applied compact status output', file=sys.stderr)
PY
