# Sensor Simulation

The NOS3 sensor suite covers attitude and navigation (IMU, star tracker, GPS, magnetometer, sun sensors) but not Earth observation payloads (SAR, thermal IR, multispectral cameras). Since [SpaceCoMP workflows](/spacecomp/use-cases/overview) depend on sensor data to detect anomalies, the simulation must inject synthetic observation data into the pipeline.

Three strategies are available, in order of increasing fidelity.

## Parametric Anomaly Models

The simplest approach. No images are generated. Instead, the simulator injects an anomaly descriptor directly into the workflow's map phase — a set of pixel coordinates and values that exceed the detection threshold. A configuration file defines anomaly events: location within the AOI, magnitude, onset time, and temporal profile (step, ramp, periodic).

This tests the workflow logic (state management, threshold evaluation, alert generation, [SRSPP](/protocols/transport/srspp) routing) without the computational cost of image processing.

## Synthetic Raster Injection

Pre-generated sensor images are loaded from disk and injected into the workflow's collect phase when the satellite passes the AOI. The workflow processes them through its full map/reduce pipeline.

The `eosim` tool (`tools/eosim/`) generates synthetic rasters for different sensor types:

- **Thermal IR** — brightness temperature rasters with a diurnal background model and configurable fire ignition points
- **SAR** — SLC images with displacement phase screens for InSAR workflows, or backscatter images with dark-spot injection for oil spill detection
- **Multispectral** — NDVI maps with per-land-cover baselines and deforestation polygons

Raster files are stored alongside the workflow definition and indexed by pass number. The simulator selects the next file each time the satellite enters the AOI.

```
cd tools/eosim
uv run eosim wildfire examples/california_wildfire.yaml -o output/ --fmt bin
```

Binary format for NOS3 injection: `[u32 width LE] [u32 height LE] [width×height f32 LE values]`.

The thermal camera simulator (`libs/nos3/components/thermal_cam/sim/`) serves these files over a virtual SPI bus. It registers as an SPI slave, gets the satellite's position from 42, detects AOI entry, and streams the next frame to the flight software on request. The workflow cFS app reads the frame through the same hwlib SPI API it would use with real hardware.

## Real Data Replay

Archived satellite data is replayed through the workflow pipeline:

- **Sentinel-1** SAR (C-band, free, global) for InSAR, oil spill, flood, and ice use cases
- **MODIS/VIIRS** thermal for wildfire detection
- **Landsat/Sentinel-2** multispectral for deforestation

Real data provides ground truth for validating detection algorithms but requires pre-processing imagery for the specific AOI and matching the simulation's orbital geometry to the data's acquisition timeline.

## Choosing a Strategy

| Strategy | What it tests | When to use |
|---|---|---|
| Parametric | Workflow logic, alert routing, state persistence | Integration tests, CI, communication path validation |
| Synthetic raster | Full pipeline including image processing | Algorithm development, per-use-case testing |
| Real data replay | Detection accuracy against ground truth | Validation before deployment, false-positive tuning |
