#!/bin/sh
set -eu

find .handoffs -type f -name 'issue-*.md' -print | while IFS= read -r capsule; do
    for heading in \
        '## Identity and scope' \
        '## Objective' \
        '## Completed' \
        '## Verification' \
        '## Failed attempts' \
        '## Next executable steps' \
        '## Capabilities required for the next Agent' \
        '## Security and privacy notes'
    do
        grep -Fqx "$heading" "$capsule" || {
            echo "$capsule is missing heading: $heading" >&2
            exit 1
        }
    done

    if grep -Eq 'TODO|ISSUE_NUMBER|AGENT_ID|CAPABILITIES|BRANCH_NAME|CHECKPOINT_PARENT|UPDATED_AT' "$capsule"; then
        echo "$capsule contains an unresolved handoff placeholder" >&2
        exit 1
    fi

    grep -Fq -- '- Credentials included: no' "$capsule" || {
        echo "$capsule must explicitly state that credentials are not included" >&2
        exit 1
    }

    if grep -Eiq '(api[_ -]?key|access[_ -]?token|client[_ -]?secret|private[_ -]?key)[[:space:]]*[:=][[:space:]]*[^[:space:]`*<]+' "$capsule"; then
        echo "$capsule may contain credential material" >&2
        exit 1
    fi
done
