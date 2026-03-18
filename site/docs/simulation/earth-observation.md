# Earth Observation

The NOS3 sensor suite covers attitude and navigation but not Earth observation payloads. Since [SpaceCoMP workflows](/spacecomp/use-cases/overview) depend on sensor data to detect anomalies, the simulation must inject synthetic observation data into the pipeline. Three strategies are available, in order of increasing fidelity.

## Parametric Anomaly Models

No images are generated. The simulator injects an anomaly descriptor directly into the workflow's map phase — a set of pixel coordinates and values that exceed the detection threshold. A configuration file defines anomaly events: location within the AOI, magnitude, onset time, and temporal profile.

This tests the workflow logic (state management, threshold evaluation, alert generation, [SRSPP](/protocols/transport/srspp) routing) without the computational cost of image processing.

## Synthetic Raster Injection

Pre-generated sensor images are loaded from disk and injected into the workflow's collect phase when the satellite passes the AOI. The workflow processes them through its full map/reduce pipeline.

The `eosim` tool (`tools/eosim/`) generates synthetic rasters:

- **Thermal IR** — brightness temperature rasters in Kelvin (32-bit float). Models the MWIR band (~3.9 μm) where fires appear at ~600 K against a ~300 K background. Includes spatial variation via low-frequency noise and configurable sensor noise (NEdT). Fire events are injected at geographic coordinates with onset pass, peak temperature, spread rate, and radial falloff.
- **SAR** — SLC images with displacement phase screens for InSAR workflows, or backscatter images with dark-spot injection for oil spill detection.
- **Multispectral** — NDVI maps with per-land-cover baselines and deforestation polygons.

The thermal camera NOS3 component serves these files over a virtual SPI bus. It detects AOI entry using the satellite's position from [42](orbital-mechanics) and loads the next frame for each pass.

## Real Data Replay

Archived satellite data is replayed through the workflow pipeline:

- **Sentinel-1** SAR (C-band, free, global) for InSAR, oil spill, flood, and ice use cases
- **MODIS/VIIRS** thermal for wildfire detection
- **Landsat/Sentinel-2** multispectral for deforestation

Real data provides ground truth for validating detection algorithms but requires pre-processing imagery for the specific AOI.

## Choosing a Strategy

| Strategy | What it tests | When to use |
|---|---|---|
| Parametric | Workflow logic, alert routing, state persistence | Integration tests, CI |
| Synthetic raster | Full pipeline including image processing | Algorithm development |
| Real data replay | Detection accuracy against ground truth | Validation before deployment |
