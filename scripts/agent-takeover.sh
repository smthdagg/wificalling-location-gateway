#!/bin/sh
set -eu

if [ "$#" -lt 4 ] || [ "$#" -gt 5 ]; then
    echo "usage: $0 <issue-number> <agent-id> <slug> <capability-csv> [ttl-minutes]" >&2
    exit 2
fi

ttl=${5:-120}
exec python3 "$(dirname "$0")/agent_state.py" takeover "$1" "$2" "$3" "$4" --ttl "$ttl"
