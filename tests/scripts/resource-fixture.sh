#!/bin/sh
set -eu
dd if=/dev/zero of="${TMPDIR:-/tmp}/wloc-resource-fixture.$$" bs=1024 count=64 >/dev/null 2>&1
rm -f "${TMPDIR:-/tmp}/wloc-resource-fixture.$$"
