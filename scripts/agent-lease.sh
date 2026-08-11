#!/bin/sh
set -eu

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
    echo "usage: $0 <issue-number> <agent-id> <capability-csv> [ttl-minutes]" >&2
    exit 2
fi

ttl=${4:-120}
exec python3 "$(dirname "$0")/agent_state.py" lease "$1" "$2" "$3" --ttl "$ttl"
