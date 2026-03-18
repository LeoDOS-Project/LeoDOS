# Flood

During flood events, rapid mapping of inundated areas guides evacuation
and relief efforts.

**Sensor:** SAR (works through clouds and at night).

**Pipeline:**
- _Collect:_ SAR image over flood-risk AOI.
- _Map:_ classify pixels as water/non-water using backscatter threshold
  (water is specularly reflective -> dark in SAR). Compare
  against pre-flood baseline to identify newly inundated areas.
- _Reduce:_ compute total flooded area, identify affected
  infrastructure (by overlaying with a stored vector map of
  roads/buildings). Generate alert.

**Alert payload:** ~5 KB (flood extent polygon, area km^2, list of
affected infrastructure IDs, timestamp).

**Feasibility:**
- Water/non-water classification in SAR is robust and simple (bimodal
  histogram thresholding).
- Infrastructure overlay requires storing a lightweight vector map
  onboard (feasible for a bounded AOI).
- Time-critical during active flood events.
