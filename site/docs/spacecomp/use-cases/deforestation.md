# Deforestation

Monitoring protected forest areas for unauthorized clearing, especially
in remote regions where ground patrols are infeasible.

**Sensor:** multispectral (visible + NIR).

**Pipeline:**
- _Collect:_ multispectral image over protected forest AOI.
- _Map:_ compute NDVI per pixel:
  $$\text{NDVI} = \frac{\text{NIR} - \text{Red}}{\text{NIR} + \text{Red}}$$
  Compare against baseline NDVI map. Flag pixels where $\Delta$NDVI
  exceeds deforestation threshold (e.g. drop of > 0.3).
- _Reduce:_ cluster changed pixels, compute area in hectares. Filter
  out seasonal variation using baseline update schedule. Generate alert
  if cleared area exceeds minimum (e.g. > 1 ha).

**Alert payload:** ~3 KB (polygon outline of cleared area, area in
hectares, NDVI delta, timestamp).

**Feasibility:**
- NDVI computation is trivial (2 bands, 1 division per pixel).
- Cloud cover is the main limitation; SAR-based alternatives
  (backscatter change detection) work through clouds but require more
  complex processing.
- Baseline must be updated seasonally to avoid false alerts from
  natural phenology changes.
