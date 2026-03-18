# Earth Observation

Earth observation imagery is the primary input for [SpaceCoMP workflows](/spacecomp/use-cases/overview). The simulation supports injecting both synthetic and real sensor data into the pipeline.

## Supported

### Thermal IR

Brightness temperature images in the mid-wave infrared (MWIR, ~3.9 μm) and long-wave infrared (LWIR, ~11 μm) bands. Thermal IR does not depend on sunlight — it measures heat radiated by the surface, so it works at night and through thin cloud. A fire at ~600 K stands out clearly against a ~300 K background. Thermal data is the primary input for [wildfire detection](/spacecomp/use-cases/wildfire) workflows.

The full pipeline is implemented end-to-end:

- **Synthetic data** — the `eosim` tool generates brightness temperature rasters with a diurnal background model, spatial variation, sensor noise (NEdT), and configurable fire events (location, onset, peak temperature, spread rate).
- **NOS3 simulator** — the thermal camera component serves frames over a virtual SPI bus, detects AOI entry using the satellite's position from [42](orbital-mechanics), and loads the appropriate frame for each pass.
- **cFS app** — the wildfire app captures frames via SPI, thresholds brightness temperature, clusters hot pixels, converts to geographic coordinates, and sends alert packets via [SRSPP](/protocols/transport/srspp) to the ground station.
- **Real data** — MODIS and VIIRS thermal bands provide global coverage at 375 m–1 km resolution with multiple daily revisits.

## Not Yet Supported

### SAR

Synthetic Aperture Radar transmits microwave pulses and records the reflected signal. SAR works through clouds, at night, and in all weather. SAR data comes in two forms:

- **Complex (SLC)** — preserves both amplitude and phase. Phase differences between repeat passes reveal sub-millimeter surface displacement (InSAR), used for [tailings dam](/spacecomp/use-cases/tailings-dam) displacement monitoring.
- **Backscatter (intensity)** — measures how strongly the surface reflects radar. Calm water appears dark (specular reflection away from the sensor), making oil spills and flood extent visible as dark patches. Used for [oil spill](/spacecomp/use-cases/oil-spill), [flood](/spacecomp/use-cases/flood), and [sea ice](/spacecomp/use-cases/sea-ice) detection.

Real data is available from Sentinel-1 (free, global C-band SAR, 5×20 m resolution, 6-day revisit). No eosim generator, NOS3 simulator, or cFS processing app exists yet.

### Multispectral

Images captured in multiple wavelength bands (visible, near-infrared, shortwave infrared). The key derived product is NDVI (Normalized Difference Vegetation Index), which measures vegetation health by comparing red and near-infrared reflectance. Healthy vegetation reflects strongly in NIR and absorbs red; a drop in NDVI over time indicates [deforestation](/spacecomp/use-cases/deforestation) or crop loss.

Real data is available from Landsat (30 m, 16-day revisit) and Sentinel-2 (10 m, 5-day revisit). No eosim generator, NOS3 simulator, or cFS processing app exists yet.

### Optical

Visible-light imagery. The Arducam (OV5640) camera has a NOS3 simulator and Rust bindings, but it is not integrated into any Earth observation workflow — no eosim generator or cFS processing app uses it for EO purposes. Optical images depend on daylight and clear skies.
