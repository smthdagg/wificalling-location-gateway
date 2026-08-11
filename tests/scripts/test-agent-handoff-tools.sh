#!/bin/sh
set -eu

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/wlg-handoff-test.XXXXXX")
trap 'rm -rf "$test_root"' EXIT HUP INT TERM

expect_failure() {
    if "$@" >/dev/null 2>&1; then
        echo "expected command to fail: $*" >&2
        exit 1
    fi
}

expect_failure "$repo_root/scripts/agent-lease.sh" not-a-number agent go
expect_failure "$repo_root/scripts/agent-takeover.sh" 1 agent INVALID go
expect_failure "$repo_root/scripts/agent-handoff.sh" 1 invalid/agent go
expect_failure "$repo_root/scripts/claim-issue.sh" 1

mkdir -p "$test_root/.handoffs"
capsule="$test_root/.handoffs/issue-42.md"
cp "$repo_root/.agents/HANDOFF_TEMPLATE.md" "$capsule"

sed \
    -e 's/ISSUE_NUMBER/42/g' \
    -e 's/AGENT_ID/test-agent/g' \
    -e 's/CAPABILITIES/go,test/g' \
    -e 's/BRANCH_NAME/codex\/issue-42-test/g' \
    -e 's/CHECKPOINT_PARENT/0123456789012345678901234567890123456789/g' \
    -e 's/UPDATED_AT/2026-08-11T00:00:00Z/g' \
    -e 's/TODO/None/g' \
    "$capsule" > "$capsule.rendered"
mv "$capsule.rendered" "$capsule"

(cd "$test_root" && "$repo_root/scripts/ci/verify-handoffs.sh")

printf '\nTODO\n' >> "$capsule"
expect_failure sh -c "cd '$test_root' && '$repo_root/scripts/ci/verify-handoffs.sh'"

echo 'agent handoff tool tests passed'
