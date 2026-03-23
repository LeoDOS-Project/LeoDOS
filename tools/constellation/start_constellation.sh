#!/bin/bash
set -e

MAX_ORB=${MAX_ORB:-3}
MAX_SAT=${MAX_SAT:-3}
TOTAL=$(( MAX_ORB * MAX_SAT ))

echo "Starting constellation: $MAX_ORB orbits x $MAX_SAT sats = $TOTAL satellites"

BASE=/cFS/build/exe/cpu1
pids=()
for orb in $(seq 0 $((MAX_ORB - 1))); do
    for sat in $(seq 1 "$MAX_SAT"); do
        SCID=$(( (orb + 1) * 1000 + sat ))
        CPUID=$(( orb * MAX_SAT + sat ))
        INST=/cFS/run/sat_${SCID}
        mkdir -p "$INST"
        cp -a "$BASE"/. "$INST"/
        echo "Launching SCID=$SCID CPUID=$CPUID (orbit=$orb, sat=$sat)"
        cd "$INST"
        ./core-cpu1 --scid "$SCID" --cpuid "$CPUID" &
        pids+=($!)
    done
done

echo "All $TOTAL satellites launched."

cleanup() {
    echo "Shutting down constellation..."
    for pid in "${pids[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait
}

trap cleanup SIGTERM SIGINT

wait
