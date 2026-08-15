#!/bin/sh
# Harden the Gateway config compiler against stale device-policy node
# references.
#
# When nodes are re-imported in LuCI their UCI section names change, and a
# device policy that still references an old section name made the compiler
# fail outright ("device references unknown node") - which prevented
# sing-box.json from being generated and stopped the proxy for EVERY
# device, not just the stale one. The compiler now skips such devices with
# a warning instead: the stale device falls back to direct routing while
# every other device keeps working.
#
# Fail-closed: any missing target string aborts the build. Idempotent.

set -eu

payload=${1:?payload directory required}
compiler="$payload/usr/libexec/wificalling-gateway/compiler.sh"

[ -f "$compiler" ] || { echo "patch-gateway-device-guard: missing $compiler" >&2; exit 2; }

python3 - "$compiler" <<'PY'
import sys

compiler = sys.argv[1]

with open(compiler, encoding='utf-8') as handle:
    text = handle.read()

if 'device_guard_marker' in text:
    print('patch-gateway-device-guard: already patched', file=sys.stderr)
    raise SystemExit(0)

old = 'if (!node_proto[$3]) fail("device references unknown node: " $3)'
new = (
    '# device_guard_marker: a stale device-policy node reference must not\n'
    '# take down the whole gateway - skip the device so the rest keeps\n'
    '# proxying (the stale device falls back to direct routing).\n'
    'if (!node_proto[$3]) { print "gateway: device references unknown node " $3 "; skipping" > "/dev/stderr"; next }'
)

if old not in text:
    print(
        'patch-gateway-device-guard: target not found; '
        'gateway version mismatch?',
        file=sys.stderr,
    )
    raise SystemExit(2)
text = text.replace(old, new, 1)

with open(compiler, 'w', encoding='utf-8') as handle:
    handle.write(text)

print('patch-gateway-device-guard: applied stale device guard', file=sys.stderr)
PY
