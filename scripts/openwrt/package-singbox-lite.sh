#!/bin/sh
set -eu

data_root=${1:-}
runtime_bin=${2:-}
expected_sha=${3:-}

fail() {
	printf 'package-singbox-lite: %s\n' "$*" >&2
	exit 2
}

[ -d "$data_root" ] || fail 'package data root must exist'
[ -x "$runtime_bin" ] || fail 'runtime binary must be executable'
case "$expected_sha" in *[!0-9a-fA-F]*|'') fail 'invalid runtime SHA-256' ;; esac
[ "${#expected_sha}" -eq 64 ] || fail 'invalid runtime SHA-256'

if command -v sha256sum >/dev/null 2>&1; then
	actual_sha=$(sha256sum "$runtime_bin" | awk '{print $1}')
else
	actual_sha=$(shasum -a 256 "$runtime_bin" | awk '{print $1}')
fi
[ "$actual_sha" = "$expected_sha" ] || fail 'runtime SHA-256 mismatch'

share_dir="$data_root/usr/share/wificalling-location-gateway"
mkdir -p "$data_root/usr/bin" "$share_dir"
gzip -9 -n -c "$runtime_bin" > "$share_dir/sing-box-lite.gz"
printf '%s\n' "$expected_sha" > "$share_dir/sing-box-lite.sha256"
chmod 0644 "$share_dir/sing-box-lite.gz" "$share_dir/sing-box-lite.sha256"

cat > "$data_root/usr/bin/sing-box" <<EOF
#!/bin/sh
set -eu

archive=/usr/share/wificalling-location-gateway/sing-box-lite.gz
expected_sha='$expected_sha'
target=/tmp/sing-box-lite
stamp=/tmp/sing-box-lite.sha256
lock_dir=/tmp/.sing-box-lite.lock

runtime_ready() {
	[ -x "\$target" ] && [ "\$(cat "\$stamp" 2>/dev/null || true)" = "\$expected_sha" ]
}

if ! runtime_ready; then
	if mkdir "\$lock_dir" 2>/dev/null; then
		candidate="\$target.new.\$\$"
		cleanup() {
			rm -f "\$candidate"
			rmdir "\$lock_dir" 2>/dev/null || true
		}
		trap cleanup EXIT HUP INT TERM
		gzip -dc "\$archive" > "\$candidate"
		if command -v sha256sum >/dev/null 2>&1; then
			actual_sha=\$(sha256sum "\$candidate" | awk '{print \$1}')
		else
			actual_sha=\$(shasum -a 256 "\$candidate" | awk '{print \$1}')
		fi
		[ "\$actual_sha" = "\$expected_sha" ] || {
			echo 'sing-box Lite runtime checksum mismatch' >&2
			exit 126
		}
		chmod 0755 "\$candidate"
		mv "\$candidate" "\$target"
		printf '%s\n' "\$expected_sha" > "\$stamp"
		rmdir "\$lock_dir"
		trap - EXIT HUP INT TERM
	else
		attempt=0
		while ! runtime_ready && [ "\$attempt" -lt 15 ]; do
			attempt=\$((attempt + 1))
			sleep 1
		done
		runtime_ready || {
			echo 'sing-box Lite runtime preparation timed out' >&2
			exit 126
		}
	fi
fi

exec "\$target" "\$@"
EOF
chmod 0755 "$data_root/usr/bin/sing-box"
