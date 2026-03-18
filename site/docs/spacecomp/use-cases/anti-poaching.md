# Anti-Poaching

Detecting unauthorized vehicle activity in protected wildlife reserves,
especially at night when poaching typically occurs.

**Sensor:** SAR (works through clouds and at night — critical for
nocturnal poaching activity).

**Pipeline:**
- _Collect:_ SAR strip over reserve boundary and access roads.
- _Map:_ change detection against a baseline backscatter image. Flag
  new high-reflectivity points on known access routes and within the
  reserve perimeter (vehicles are strong SAR reflectors due to corner
  reflector geometry).
- _Reduce:_ filter detections by time of day (nighttime = elevated
  suspicion), location (inside reserve boundary or on unauthorized
  access routes), and cluster size. Generate alert with coordinates,
  estimated vehicle count, and heading from track direction.

**Alert payload:** ~2 KB (detection coordinates, time, number of
vehicles, heading estimate, confidence score).

**Feasibility:**
- Vehicle detection in SAR is well-established — vehicles are
  bright point targets against natural backgrounds.
- Person-scale detection is not feasible from LEO (requires
  < 5 m GSD; typical SAR is 10--20 m). The approach detects
  _vehicles_, not individuals.
- Nighttime filtering significantly reduces false positives from
  legitimate daytime traffic (rangers, tourists).
- Repeat-pass change detection catches new tracks in soft terrain
  (sand, mud) even after vehicles have left.
- Revisit frequency (90 min orbit period) limits real-time tracking
  but suffices for alerting rangers to investigate.
