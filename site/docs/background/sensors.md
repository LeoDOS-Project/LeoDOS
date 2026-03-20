# Sensors

LEO satellites carry sensors that capture different types of data about the Earth's surface. Each sensor type works differently and reveals different information. For details on how these are simulated in LeoDOS, see [Earth Observation](/simulation/earth-observation).

## Optical

Visible-light cameras that work like a high-resolution digital camera in space. They capture what the human eye would see — land cover, urban areas, vegetation, water bodies. Resolution ranges from 30 cm (commercial) to 10–60 m (Sentinel-2). Optical imagery depends on daylight and clear skies — clouds and darkness block the view entirely.

## Synthetic Aperture Radar (SAR)

An active sensor that transmits microwave pulses and records the reflected signal. Because it provides its own illumination, SAR works through clouds, at night, and in all weather. The satellite's motion along its orbit synthetically creates a large antenna aperture, achieving meter-scale resolution from hundreds of kilometers away.

SAR data comes in two forms:
- **Complex (SLC)** — records both the amplitude (how strongly the surface reflects) and the phase (the precise timing of the return signal). Phase differences between two passes over the same area reveal sub-millimeter surface displacement — this is InSAR (Interferometric SAR), used for monitoring ground deformation near mines, volcanoes, and infrastructure.
- **Backscatter (intensity)** — only the amplitude. Smooth surfaces like calm water reflect the radar away from the sensor and appear dark. Rough surfaces scatter the signal back and appear bright. This contrast makes oil spills, floods, and sea ice boundaries visible.

## Thermal Infrared

Measures heat radiated by the surface, not reflected sunlight. Works at night and through thin cloud. Every surface emits thermal radiation proportional to its temperature — a fire at ~600 K stands out against a ~300 K background. Thermal sensors operate in the mid-wave infrared (MWIR, ~3.9 μm) for fire detection and the long-wave infrared (LWIR, ~11 μm) for surface temperature mapping.

## Multispectral

Captures images in multiple wavelength bands simultaneously — visible, near-infrared, and shortwave infrared. Different materials reflect different wavelengths differently. The key derived product is NDVI (Normalized Difference Vegetation Index): healthy vegetation reflects strongly in near-infrared and absorbs red light, so the ratio between these bands measures vegetation health. A drop in NDVI over time indicates deforestation, drought, or crop failure.

## Hyperspectral

Similar to multispectral but with tens to hundreds of narrow bands instead of a handful of broad ones. This produces a detailed spectral fingerprint for each pixel, enabling identification of specific minerals, chemicals, or plant species. The data volume is much larger — a hyperspectral cube can be gigabytes for a single scene — making onboard [compression](/protocols/coding/compression/hyperspectral) essential.
