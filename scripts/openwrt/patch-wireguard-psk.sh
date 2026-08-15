#!/bin/sh
# Apply the WireGuard pre_shared_key support patch to a merged Gateway
# payload (usr/libexec/wificalling-gateway/compiler.sh and
# etc/init.d/wificalling-gateway).
#
# The Gateway 1.7.x config compiler emits sing-box wireguard endpoints
# without the peer pre-shared key, so nodes that require a PSK can never
# complete a handshake. This patch adds an optional `pre_shared_key` UCI
# option (field 25 of normalized.conf) and emits it into the peer block.
#
# The patch is fail-closed: if any target string is missing (a different
# Gateway version was merged), the build aborts instead of silently
# shipping a wireguard node that cannot connect. It is idempotent.

set -eu

payload=${1:?payload directory required}
compiler="$payload/usr/libexec/wificalling-gateway/compiler.sh"
initd="$payload/etc/init.d/wificalling-gateway"

[ -f "$compiler" ] || { echo "patch-wireguard-psk: missing $compiler" >&2; exit 2; }
[ -f "$initd" ] || { echo "patch-wireguard-psk: missing $initd" >&2; exit 2; }

python3 - "$compiler" "$initd" <<'PY'
import sys

compiler, initd = sys.argv[1], sys.argv[2]


def apply(path, edits, name, patched_marker):
    with open(path, encoding='utf-8') as handle:
        text = handle.read()
    if patched_marker in text:
        return  # already patched
    for old, new in edits:
        if old not in text:
            print(
                f'patch-wireguard-psk: {name} target not found; '
                'gateway version mismatch?',
                file=sys.stderr,
            )
            raise SystemExit(2)
        text = text.replace(old, new, 1)
    with open(path, 'w', encoding='utf-8') as handle:
        handle.write(text)


compiler_edits = [
    # endpoint style (sing-box >= 1.11)
    (
        's=s ",\\"peers\\":[{\\"address\\":" q(f[4]) ",\\"port\\":" f[5] ",\\"public_key\\":" q(f[13]) ",\\"allowed_ips\\":[\\"0.0.0.0/0\\"]"',
        's=s ",\\"peers\\":[{\\"address\\":" q(f[4]) ",\\"port\\":" f[5] ",\\"public_key\\":" q(f[13]) ",\\"allowed_ips\\":[\\"0.0.0.0/0\\"]"\n'
        '      if (f[25]!="") s=s ",\\"pre_shared_key\\":" q(f[25])',
    ),
    # legacy outbound style (sing-box < 1.11)
    (
        's=s ",\\"private_key\\":" q(f[21]) ",\\"peer_public_key\\":" q(f[13]) ",\\"local_address\\":[" q(f[22]) "]"',
        's=s ",\\"private_key\\":" q(f[21]) ",\\"peer_public_key\\":" q(f[13]) ",\\"local_address\\":[" q(f[22]) "]"\n'
        '      if (f[25]!="") s=s ",\\"pre_shared_key\\":" q(f[25])',
    ),
]
apply(compiler, compiler_edits, 'compiler.sh', 'f[25]')

initd_edits = [
    (
        'config_get private_key "$s" private_key; config_get local_address "$s" local_address; config_get reserved "$s" reserved; config_get mtu "$s" mtu',
        'config_get private_key "$s" private_key; config_get local_address "$s" local_address; config_get reserved "$s" reserved; config_get mtu "$s" mtu; config_get pre_shared_key "$s" pre_shared_key',
    ),
    (
        "printf 'node|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s\\n' \"$s\" \"$protocol\" \"$server\" \"$port\" \"$credential\" \"$sni\" \"$insecure\" \"$alpn\" \"$auxiliary\" \"$congestion\" \"$udp_mode\" \"$public_key\" \"$short_id\" \"$fingerprint\" \"$security\" \"$transport\" \"$path\" \"$host\" \"$pin_sha256\" \"$private_key\" \"$local_address\" \"$reserved\" \"$mtu\"",
        "printf 'node|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s\\n' \"$s\" \"$protocol\" \"$server\" \"$port\" \"$credential\" \"$sni\" \"$insecure\" \"$alpn\" \"$auxiliary\" \"$congestion\" \"$udp_mode\" \"$public_key\" \"$short_id\" \"$fingerprint\" \"$security\" \"$transport\" \"$path\" \"$host\" \"$pin_sha256\" \"$private_key\" \"$local_address\" \"$reserved\" \"$mtu\" \"$pre_shared_key\"",
    ),
]
apply(initd, initd_edits, 'init.d', 'config_get pre_shared_key')

print('patch-wireguard-psk: applied pre_shared_key support', file=sys.stderr)
PY
