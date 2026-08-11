#!/bin/sh
set -eu

if [ "$#" -ne 3 ]; then
    echo "usage: $0 <issue-number> <agent-name> <slug>" >&2
    exit 2
fi

echo 'note: this is a low-level fresh-work helper; prefer agent-takeover.sh for resumable tasks' >&2

issue=$1
agent=$2
slug=$3

case "$issue" in *[!0-9]*|'') echo 'issue-number must be numeric' >&2; exit 2 ;; esac
case "$agent" in *[!A-Za-z0-9_-]*|'') echo 'agent-name contains invalid characters' >&2; exit 2 ;; esac
case "$slug" in *[!a-z0-9-]*|'') echo 'slug must contain lowercase letters, digits, or hyphens' >&2; exit 2 ;; esac

repo_root=$(git rev-parse --show-toplevel)
repo_parent=$(dirname "$repo_root")
branch="codex/issue-$issue-$slug"
worktree="$repo_parent/wlg-agent-$issue-$agent"

if [ -e "$worktree" ]; then
    echo "worktree path already exists: $worktree" >&2
    exit 1
fi

git fetch origin main
git worktree add -b "$branch" "$worktree" origin/main

printf 'created worktree: %s\nbranch: %s\n' "$worktree" "$branch"
