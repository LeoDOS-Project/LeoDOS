# Wildfire

Early wildfire detection (minutes vs hours) saves lives and reduces
suppression costs by orders of magnitude.

**Sensor:** thermal IR (MWIR/LWIR bands).

**Pipeline:**
- _Collect:_ thermal imagery over monitored fire-risk zones.
- _Map:_ for each pixel, compute brightness temperature. Flag pixels
  where $T >$ contextual threshold (accounting for solar heating, land
  cover). Compare against a running background model.
- _Reduce:_ cluster adjacent hot pixels. Filter false positives (sun
  glint, industrial heat). Generate alert if cluster exceeds minimum
  size and persistence (confirmed on 2+ consecutive frames if
  available).

**Alert payload:** ~1 KB (coordinates, temperature, cluster size,
confidence, timestamp).

**Feasibility:**
- Thermal anomaly detection is well-understood (MODIS, VIIRS algorithms
  exist).
- Computational cost is low (thresholding + clustering).
- Time-critical: minutes matter. Onboard processing eliminates the
  ground-processing delay entirely.
- False positive rate is the main challenge; contextual algorithms help
  but are more compute-intensive.
