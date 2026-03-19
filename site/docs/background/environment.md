# Environment

The space environment imposes constraints that do not exist in ground-based computing. Radiation damages electronics, atmospheric drag limits satellite lifetime, orbital debris threatens physical survival, and the downlink bottleneck limits how much data reaches the ground.

## Earth Observation Data

LEO satellites carry sensors that capture different types of data about the Earth's surface. Each sensor type works differently and reveals different information. For details on how these are simulated in LeoDOS, see [Earth Observation](/simulation/earth-observation).

### Optical

Visible-light cameras that work like a high-resolution digital camera in space. They capture what the human eye would see — land cover, urban areas, vegetation, water bodies. Resolution ranges from 30 cm (commercial) to 10–60 m (Sentinel-2). Optical imagery depends on daylight and clear skies — clouds and darkness block the view entirely.

### Synthetic Aperture Radar (SAR)

An active sensor that transmits microwave pulses and records the reflected signal. Because it provides its own illumination, SAR works through clouds, at night, and in all weather. The satellite's motion along its orbit synthetically creates a large antenna aperture, achieving meter-scale resolution from hundreds of kilometers away.

SAR data comes in two forms:
- **Complex (SLC)** — records both the amplitude (how strongly the surface reflects) and the phase (the precise timing of the return signal). Phase differences between two passes over the same area reveal sub-millimeter surface displacement — this is InSAR (Interferometric SAR), used for monitoring ground deformation near mines, volcanoes, and infrastructure.
- **Backscatter (intensity)** — only the amplitude. Smooth surfaces like calm water reflect the radar away from the sensor and appear dark. Rough surfaces scatter the signal back and appear bright. This contrast makes oil spills, floods, and sea ice boundaries visible.

### Thermal Infrared

Measures heat radiated by the surface, not reflected sunlight. Works at night and through thin cloud. Every surface emits thermal radiation proportional to its temperature — a fire at ~600 K stands out against a ~300 K background. Thermal sensors operate in the mid-wave infrared (MWIR, ~3.9 μm) for fire detection and the long-wave infrared (LWIR, ~11 μm) for surface temperature mapping.

### Multispectral

Captures images in multiple wavelength bands simultaneously — visible, near-infrared, and shortwave infrared. Different materials reflect different wavelengths differently. The key derived product is NDVI (Normalized Difference Vegetation Index): healthy vegetation reflects strongly in near-infrared and absorbs red light, so the ratio between these bands measures vegetation health. A drop in NDVI over time indicates deforestation, drought, or crop failure.

### Hyperspectral

Similar to multispectral but with tens to hundreds of narrow bands instead of a handful of broad ones. This produces a detailed spectral fingerprint for each pixel, enabling identification of specific minerals, chemicals, or plant species. The data volume is much larger — a hyperspectral cube can be gigabytes for a single scene — making onboard [compression](/protocols/coding/compression/hyperspectral) essential.

## The Downlink Wall

LEO Earth observation satellites generate 1–2 TB of sensor data per day. Ground contact windows allow only a fraction of this to be downlinked. This is the fundamental problem LeoDOS addresses: process data onboard and downlink only the results.

| What | Size | Example |
|---|---|---|
| Raw SAR strip | ~2 GB | Full resolution radar image over a dam |
| Alert packet | ~2 KB | "Displacement exceeds 5 mm at these coordinates" |
| Reduction factor | ~10⁶ | Processing onboard avoids downlinking data the ground doesn't need |

## Radiation

LEO satellites are exposed to ionizing radiation from three sources:

- **Van Allen radiation belts** — regions of trapped charged particles (protons and electrons) held by Earth's magnetic field. The inner belt (1,000–6,000 km) contains high-energy protons; the outer belt (13,000–60,000 km) contains electrons. LEO satellites fly below the inner belt but pass through the **South Atlantic Anomaly (SAA)**, where the inner belt dips to ~200 km altitude due to the offset between Earth's geographic and magnetic poles. The SAA is the primary radiation concern for LEO missions.
- **Galactic cosmic rays** — high-energy particles from outside the solar system. Low flux but very penetrating. Cannot be shielded effectively.
- **Solar particle events** — bursts of protons from solar flares. Intermittent but can deliver a large dose in hours.

Radiation causes:
- **Single-event upsets (SEUs)** — a particle flips a bit in memory or a register. Addressed by ECC memory and software checksums (see [fault tolerance](/cfs/mission/fault-tolerance)).
- **Total ionizing dose (TID)** — cumulative damage to transistors over the mission lifetime. Addressed by radiation-hardened processor design.
- **Single-event latchups** — a particle triggers a short circuit that can only be cleared by a power cycle.

## Atmospheric Drag

At LEO altitudes, especially below 500 km, residual atmosphere exerts drag on the satellite. Drag lowers the orbit over time — without periodic orbit-raising maneuvers (using thrusters), the satellite eventually reenters the atmosphere.

Drag depends on:
- **Altitude** — drag decreases roughly exponentially with altitude. At 200 km, a satellite reenters in days; at 600 km, it can last decades.
- **Solar activity** — the Sun heats the upper atmosphere, causing it to expand. During solar maximum, drag at 400 km can be 10× higher than during solar minimum.
- **Ballistic coefficient** — the satellite's mass-to-area ratio. A large, lightweight satellite (like one with deployed solar panels) experiences more drag.

## Orbital Debris

There are over 30,000 tracked debris objects in orbit, and millions of smaller untracked fragments. A collision at orbital velocity (7–8 km/s in LEO) can destroy a satellite and generate hundreds of new fragments, each capable of destroying another satellite. This cascading effect is called the **Kessler syndrome**.

Conjunction assessment — predicting close approaches between objects — is a routine part of constellation operations. When a close approach is predicted, the satellite can perform a collision avoidance maneuver (raising or lowering its orbit slightly to increase the miss distance).

## Flight Software

Satellites run real-time flight software on radiation-hardened [processors](/cfs/mission/processor). The software manages all onboard operations: attitude control, power management, communication, payload data processing. LeoDOS uses NASA's [Core Flight System](/cfs/overview) as the flight software framework, with applications written in Rust.
