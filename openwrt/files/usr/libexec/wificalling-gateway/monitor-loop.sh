#!/bin/sh
clients=$1; output=$2; nodes=$3; node_output=$4; events=$5; state=$6; event_interval=${7:-60}; max_events=${8:-20}; log_enabled=${9:-1}; tick=0
cursor="${state}.node-health"

next_node() {
	last=$(cat "$cursor" 2>/dev/null || true)
	awk -F'|' -v last="$last" '
		$1 != "" {
			if (after) { found = 1; print $1; exit }
			if (!first) first = $1
			if ($1 == last) after = 1
		}
		END { if (!found && first) print first }
	' "$nodes"
}

while :; do
	/usr/libexec/wificalling-gateway/monitor.sh "$clients" /proc/net/nf_conntrack "$output" "$state" "$events" "$event_interval" "$max_events" "$log_enabled"
	if [ "$tick" -eq 0 ]; then
		node=$(next_node)
		if [ -n "$node" ]; then
			/usr/libexec/wificalling-gateway/node-health.sh "$nodes" "$node_output" "$node"
			printf '%s\n' "$node" > "$cursor"
		fi
	fi
	tick=$(( (tick + 1) % 2 ))
	sleep 5
done
