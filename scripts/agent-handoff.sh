#!/bin/sh
set -eu

if [ "$#" -ne 3 ]; then
    echo "usage: $0 <issue-number> <agent-id> <capability-csv>" >&2
    exit 2
fi

exec python3 "$(dirname "$0")/agent_state.py" handoff "$1" "$2" "$3"
