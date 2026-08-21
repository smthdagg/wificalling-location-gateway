#!/bin/sh
set -eu
dd if=/dev/zero of="${TMPDIR:-/tmp}/wloc-resource-fixture.$$" bs=1024 count=64 >/dev/null 2>&1
rm -f "${TMPDIR:-/tmp}/wloc-resource-fixture.$$"
# Keep the deterministic fixture alive long enough for procfs fallback
# samplers to observe a non-zero RSS value on a busy CI host.
sleep 1
