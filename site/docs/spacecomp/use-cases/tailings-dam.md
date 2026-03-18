# Tailings Dam

Tailings dams at mining sites can fail catastrophically if ground
displacement exceeds safe limits. Traditional InSAR monitoring requires
ground processing with days of latency.

**Sensor:** SAR (Synthetic Aperture Radar).

**Pipeline:**
- _Collect:_ SAR strip over the dam AOI.
- _Map:_ interferometric phase subtraction against a stored master
  image (differential InSAR). For each pixel, compute displacement:
  $$\Delta d = \frac{\Delta \phi \cdot \lambda}{4 \pi}$$
- _Reduce:_ count pixels where $|\Delta d| >$ threshold (e.g. 5 mm). If
  count exceeds minimum cluster size, generate alert with centroid
  coordinates, max displacement, and affected area.

**Alert payload:** ~2 KB (AOI ID, timestamp, centroid lat/lon, max
displacement mm, pixel count, confidence score).

**Feasibility:**
- Differential InSAR phase subtraction is computationally simple
  (complex multiply + angle extraction per pixel).
- The master image (~2 GB compressed SLC) must be stored onboard —
  feasible with modern flash storage.
- Atmospheric phase screen correction is the hard part; a simplified
  threshold-based approach (flag only large displacements) avoids
  needing full atmospheric modeling.
- Repeat-pass interval depends on orbit: ~daily for a large
  constellation, weekly for smaller ones.
