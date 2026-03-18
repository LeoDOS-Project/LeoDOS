# Earth Observation

The NOS3 sensor suite covers attitude and navigation but not Earth observation payloads (SAR, thermal IR, multispectral). Three strategies inject synthetic observation data for [workflow testing](/spacecomp/use-cases/overview):

## Parametric

No images generated. The simulator injects anomaly descriptors (coordinates + values) directly into the workflow's map phase. A configuration file defines events: location, magnitude, onset time, temporal profile.

Tests workflow logic (state management, thresholds, alert routing) without image processing cost.

## Synthetic Raster

Pre-generated sensor images loaded from disk when the satellite passes the AOI. The `eosim` tool (`tools/eosim/`) generates:

- **Thermal IR** — brightness temperature rasters (MWIR ~3.9 μm). Fires at ~600 K against ~300 K background, with spatial noise and configurable fire events (location, onset, peak temperature, spread rate, falloff).
- **SAR** — SLC images with displacement phase screens (InSAR) or backscatter images with dark spots (oil spill).
- **Multispectral** — NDVI maps with per-land-cover baselines and deforestation polygons.

The thermal camera NOS3 component detects AOI entry using position from [42](orbital-mechanics) and serves the next frame for each pass.

## Real Data Replay

Archived satellite data replayed through the pipeline:

- **Sentinel-1** SAR for InSAR, oil spill, flood, and ice
- **MODIS/VIIRS** thermal for wildfire
- **Landsat/Sentinel-2** multispectral for deforestation

Provides ground truth for validating detection algorithms.

## Choosing a Strategy

| Strategy | Tests | When to use |
|---|---|---|
| Parametric | Workflow logic, alert routing | Integration tests, CI |
| Synthetic raster | Full pipeline with image processing | Algorithm development |
| Real data replay | Detection accuracy vs ground truth | Pre-deployment validation |
