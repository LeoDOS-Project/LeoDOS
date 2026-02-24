#!/bin/bash
set -e

ORBIT_ID=${ORBIT_ID:-0}
MAX_SAT=${MAX_SAT:-3}

echo "Starting orbit $ORBIT_ID with $MAX_SAT satellites"

pids=()
for sat in $(seq 1 "$MAX_SAT"); do
    SCID=$(( (ORBIT_ID + 1) * 1000 + sat ))
    CPUID=$(( ORBIT_ID * MAX_SAT + sat ))
    echo "Launching satellite SCID=$SCID CPUID=$CPUID (orbit=$ORBIT_ID, sat=$sat)"
    cd /cFS/build/exe/cpu1
    ./core-cpu1 --scid "$SCID" --cpuid "$CPUID" &
    pids+=($!)
done

cleanup() {
    echo "Shutting down orbit $ORBIT_ID..."
    for pid in "${pids[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait
}

trap cleanup SIGTERM SIGINT

wait
