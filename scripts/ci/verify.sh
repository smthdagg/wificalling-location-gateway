#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo_root"

required_files='AGENTS.md DEVELOPMENT_TEST_PLAN.md SECURITY.md .github/CODEOWNERS .github/PULL_REQUEST_TEMPLATE.md .github/ISSUE_TEMPLATE/agent-task.yml'
for required_file in $required_files; do
    if [ ! -s "$required_file" ]; then
        echo "missing required file: $required_file" >&2
        exit 1
    fi
done

find scripts -type f -name '*.sh' -print | while IFS= read -r script; do
    sh -n "$script"
done

./scripts/ci/verify-handoffs.sh
./tests/scripts/test-agent-handoff-tools.sh
./tests/scripts/test-verify-rust-openwrt.sh
./tests/scripts/test-openwrt-release-packaging.sh
./tests/scripts/test-package-variants.sh
./tests/scripts/test-standalone-ax6s-package.sh
./tests/scripts/test-release-version.sh
./tests/scripts/test-monitor-temp-cleanup.sh
./tests/scripts/test-gateway-health-report.sh
./tests/scripts/test-wloc-runtime-contract.sh
./tests/scripts/test-wloc-synthesis-default.sh
./tests/scripts/test-wloc-listener-nonblocking.sh
python3 -m unittest discover -s tests -p 'test_*.py'
python3 ./scripts/scan_secrets.py

# LuCI view regression guards (device status, CA profile auto-regen, i18n).
for js_test in tests/js/*.test.js; do
    node "$js_test"
done

if command -v shellcheck >/dev/null 2>&1; then
    find scripts -type f -name '*.sh' -exec shellcheck {} +
fi

if [ -f go.mod ]; then
    test -z "$(gofmt -l .)" || {
        echo 'gofmt changes required' >&2
        gofmt -l . >&2
        exit 1
    }
    go test ./...
fi

if [ -f Cargo.toml ]; then
    ./scripts/ci/verify-rust.sh
fi

for forbidden in '*.key' '*.p12' '*.pfx'; do
    if find . -path './.git' -prune -o -type f -name "$forbidden" -print | grep -q .; then
        echo "forbidden secret-like file found: $forbidden" >&2
        exit 1
    fi
done

echo 'repository gates passed'
