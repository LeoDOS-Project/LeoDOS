#!/bin/bash
set -e

MAX_ORB=${MAX_ORB:-3}
MAX_SAT=${MAX_SAT:-3}
OUTPUT=${OUTPUT:-docker-compose.constellation.yml}

echo "Generating constellation: $MAX_ORB orbits x $MAX_SAT sats -> $OUTPUT"

cat > "$OUTPUT" << 'HEADER'
networks:
  leodos:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16

services:
HEADER

for orb in $(seq 0 $((MAX_ORB - 1))); do
    cat >> "$OUTPUT" << EOF
  orb-${orb}:
    image: leodos-sat:latest
    hostname: orb-${orb}
    networks:
      leodos:
        ipv4_address: 172.20.${orb}.10
    environment:
      - ORBIT_ID=${orb}
      - MAX_SAT=${MAX_SAT}
      - MAX_ORB=${MAX_ORB}
    entrypoint: /start_orbit.sh
    sysctls:
      - fs.mqueue.msg_max=1000

EOF
done

echo "Generated $OUTPUT with $MAX_ORB orbit containers"
