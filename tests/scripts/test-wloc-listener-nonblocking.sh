#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
source_file="$repo_root/src/bin/wloc-service.rs"

grep -F 'std_listener.set_nonblocking(true)?;' "$source_file" >/dev/null || {
	echo 'wloc TPROXY listener must be nonblocking before Tokio takes ownership' >&2
	exit 1
}

set_line=$(grep -nF 'std_listener.set_nonblocking(true)?;' "$source_file" | head -n 1 | cut -d: -f1)
from_line=$(grep -nF 'tokio::net::TcpListener::from_std(std_listener)' "$source_file" | head -n 1 | cut -d: -f1)
[ "$set_line" -lt "$from_line" ] || {
	echo 'nonblocking mode must be set before TcpListener::from_std' >&2
	exit 1
}

echo 'wloc listener nonblocking regression test passed'
