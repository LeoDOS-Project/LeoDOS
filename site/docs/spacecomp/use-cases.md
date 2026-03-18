# Critical Workflows & Anomaly Detection

Extension of the SpaceCoMP Collect-Map-Reduce model from one-shot
queries to continuous, orbit-recurring workflows with onboard change
detection and autonomous alerting.

## Motivation

SpaceCoMP today processes a single ground-initiated request: the ground
station submits a job, satellites collect/map/reduce, and the result is
downlinked on the next pass. This is sufficient for on-demand queries
but cannot handle scenarios that require _continuous monitoring_ ---
detecting a wildfire ignition, tracking ground displacement over weeks,
or spotting an oil spill before it reaches shore.

A **workflow** is a standing order: "monitor this AOI, run this pipeline
on every pass, and alert me if something triggers." The key difference
is that the constellation retains state (baselines, thresholds,
accumulated history) and acts autonomously between ground contacts.

### Why onboard?

The downlink bottleneck makes ground-based monitoring impractical for
time-critical detection:

- Satellite sensors generate TB/day; RF downlink is < 1 Gbps during
  5--15 min ground passes.
- A raw SAR strip over a tailings dam is ~2 GB; an alert packet saying
  "displacement exceeds 5 mm at these pixels" is ~2 KB --- a $10^6$
  reduction.
- For wildfire ignition, hours of latency (waiting for the next ground
  pass to downlink raw data) can mean the difference between a
  contained fire and a catastrophe.

## Workflow Structure

A workflow extends the Collect-Map-Reduce pipeline with **persistence**
and **conditional downlink**.

### Lifecycle

1. **Registration.** Ground uploads a workflow definition: AOI bounding
   box, sensor mode, map/reduce functions (or built-in pipeline ID),
   thresholds, and an initial baseline (or "acquire baseline on first
   pass").

2. **Baseline acquisition.** On the first qualifying pass, collectors
   acquire data and store it as the reference. No alert is generated.

3. **Monitoring loop.** On each subsequent pass over the AOI:
   - _Collect:_ acquire fresh sensor data.
   - _Map:_ compare against baseline (per-pixel or per-patch), flag
     anomalies exceeding threshold.
   - _Reduce:_ aggregate flagged pixels into a compact alert descriptor
     (location, magnitude, confidence).
   - _Decision:_ if anomalies found, route alert packet to the next LOS
     node for priority downlink.

4. **Baseline update.** Optionally, the baseline can be refreshed
   periodically (e.g. every _N_ passes) to account for seasonal
   changes, or the ground can upload a new one.

5. **Deregistration.** Ground sends a cancel command, or the workflow
   expires after a configured duration.

### State Management

Workflow state must survive across orbits (~90 min period). Options:

- **CDS (Critical Data Store):** cFE's built-in mechanism for persisting
  data across resets. Limited size but appropriate for thresholds,
  metadata, and small baselines.
- **Onboard filesystem:** for larger baselines (SAR reference images).
  Stored on the satellite's flash/RAM disk, referenced by workflow ID.
- **Distributed state:** for workflows spanning multiple satellites, the
  baseline could be partitioned across the AOI's collector nodes (each
  stores its own tile).

## Use Cases

### Tailings Dam Displacement Monitoring

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
- The master image (~2 GB compressed SLC) must be stored onboard ---
  feasible with modern flash storage.
- Atmospheric phase screen correction is the hard part; a simplified
  threshold-based approach (flag only large displacements) avoids
  needing full atmospheric modeling.
- Repeat-pass interval depends on orbit: ~daily for a large
  constellation, weekly for smaller ones.

### Wildfire Ignition Detection

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

### Illegal Deforestation Detection

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

### Oil Spill Detection

Early detection of oil spills at sea enables faster response and
reduces environmental damage.

**Sensor:** SAR (C-band or X-band).

**Pipeline:**
- _Collect:_ SAR image over monitored shipping lanes or offshore
  platforms.
- _Map:_ detect dark spots on ocean surface (oil dampens capillary
  waves, reducing SAR backscatter). Apply adaptive threshold relative
  to surrounding sea state.
- _Reduce:_ classify dark spots by shape (elongated = likely spill,
  circular = natural slick/low-wind zone). Filter by area (> minimum
  spill size). Generate alert with spill extent estimate.

**Alert payload:** ~2 KB (centroid, estimated area km^2, elongation
ratio, heading, wind speed context, timestamp).

**Feasibility:**
- Dark-spot detection in SAR is computationally cheap (thresholding +
  connected components).
- False positive discrimination (oil vs lookalikes like algae, low-wind
  zones) is the hard part; simple shape heuristics help, ML-based
  classifiers need more compute.
- No baseline needed --- each image is self-contained.

### Flood Extent Monitoring

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

### Volcanic Ground Deformation

Same InSAR technique as tailings dams, different AOI and threshold
parameters. Active volcanoes show cm-scale inflation in the months
before eruption. Operationally identical pipeline.

### Sea Ice Breakup Alerts

Arctic shipping lanes need advance warning of ice breakup events.

**Sensor:** SAR.

**Pipeline:**
- _Collect:_ SAR strip over monitored Arctic corridor.
- _Map:_ edge detection on ice boundaries. Compare against previous
  pass to detect fracture propagation.
- _Reduce:_ if fracture rate exceeds threshold or new leads (open water
  channels) appear in shipping lanes, generate navigation alert.

### Anti-Poaching Vehicle Detection

Detecting unauthorized vehicle activity in protected wildlife reserves,
especially at night when poaching typically occurs.

**Sensor:** SAR (works through clouds and at night --- critical for
nocturnal poaching activity).

**Pipeline:**
- _Collect:_ SAR strip over reserve boundary and access roads.
- _Map:_ change detection against a baseline backscatter image. Flag
  new high-reflectivity points on known access routes and within the
  reserve perimeter (vehicles are strong SAR reflectors due to corner
  reflector geometry).
- _Reduce:_ filter detections by time of day (nighttime = elevated
  suspicion), location (inside reserve boundary or on unauthorized
  access routes), and cluster size. Generate alert with coordinates,
  estimated vehicle count, and heading from track direction.

**Alert payload:** ~2 KB (detection coordinates, time, number of
vehicles, heading estimate, confidence score).

**Feasibility:**
- Vehicle detection in SAR is well-established --- vehicles are
  bright point targets against natural backgrounds.
- Person-scale detection is not feasible from LEO (requires
  < 5 m GSD; typical SAR is 10--20 m). The approach detects
  _vehicles_, not individuals.
- Nighttime filtering significantly reduces false positives from
  legitimate daytime traffic (rangers, tourists).
- Repeat-pass change detection catches new tracks in soft terrain
  (sand, mud) even after vehicles have left.
- Revisit frequency (90 min orbit period) limits real-time tracking
  but suffices for alerting rangers to investigate.

## Common Pattern

All use cases share the same abstract workflow:

| Phase | Operation | Output |
|---|---|---|
| Collect | Acquire sensor data over AOI | Raw image |
| Map | Compare against baseline/model | Anomaly mask |
| Reduce | Cluster + filter anomalies | Alert packet |
| Decide | Anomalies found? | Downlink or skip |

The **bandwidth reduction** is extreme in all cases: raw sensor data (GB)
-> alert packet (KB). This is precisely what makes onboard
processing worthwhile despite the limited compute available on
radiation-hardened processors.

## Implementation Considerations

### Workflow Definition Format

A workflow could be defined as a simple descriptor:

```yaml
workflow:
  id: "tailings-dam-01"
  aoi: [-23.45, -43.12, -23.40, -43.08]
  sensor: SAR
  pipeline: differential-insar
  threshold_mm: 5.0
  min_cluster_pixels: 50
  baseline: acquire-on-first-pass
  baseline_refresh: 30  # days
  alert_priority: HIGH
  expiry: 2026-06-01
```

### Onboard Compute Budget

Rough estimates for a single workflow execution on a radiation-hardened
processor (e.g. GR740, ~300 DMIPS):

| Use case | Data size | Map complexity | Time |
|---|---|---|---|
| Tailings InSAR | 2 GB SLC | Complex multiply/px | ~60 s |
| Wildfire thermal | 200 MB | Threshold/px | ~2 s |
| Deforestation | 500 MB | NDVI subtract/px | ~5 s |
| Oil spill SAR | 1 GB | Threshold + CC | ~10 s |
| Flood SAR | 1 GB | Threshold + overlay | ~15 s |
| Anti-poaching SAR | 1 GB | Change detect + filter | ~10 s |

These are within the orbital time budget (satellites are over any given
AOI for 2--5 minutes per pass, but processing can continue after the
collection window closes).

### SRSPP Integration

Alert packets are small enough to fit in a single SRSPP segment. The
workflow system would:

1. Use the existing Router to forward alerts toward the LOS node.
2. Use SRSPP's reliable transport to guarantee delivery.
3. Tag alerts with priority to enable preemptive scheduling over routine
   telemetry.

---

## Simulation

The LeoDOS simulator provides the infrastructure needed to develop and
test workflow applications without real satellites or sensors. The main
gap is _Earth observation sensor simulation_: the existing NOS3 sensor
suite covers attitude and navigation (IMU, star tracker, GPS,
magnetometer, sun sensors) but not imaging payloads (SAR, thermal IR,
multispectral cameras). This section describes how to bridge that gap.

### AOI Pass Detection

The workflow app determines AOI intersection onboard. Each satellite
knows its own position from the GPS receiver (via the NOS3 NovAtel
simulator in simulation, or real hardware in flight). The app converts
ECEF position to geodetic coordinates and checks whether the
sub-satellite point falls within a workflow's AOI bounding box. When
it does, the collection window opens.

This is a flight-realistic approach: the detection logic runs in the
cFS app itself, not in an external simulator component. In simulation,
the GPS position comes from 42's orbital propagation through the NOS3
sensor chain; in flight, it comes from the real GPS receiver. The
workflow app code is identical in both cases.

### Sensor Data Simulation

Three strategies for simulating Earth observation data, in order of
increasing fidelity:

#### Parametric Anomaly Models

The simplest approach. No images are generated. Instead, the simulator
injects an _anomaly descriptor_ directly into the workflow's map
phase:

- A configuration file defines anomaly events: location within the
  AOI, magnitude, onset time, and temporal profile (step, ramp,
  periodic).
- When the satellite passes over the AOI, the simulator evaluates the
  anomaly model at the current simulation time and produces a
  synthetic anomaly mask (pixel coordinates + values exceeding
  threshold).
- The workflow's reduce phase runs on real data --- it receives the
  mask and produces an alert packet as normal.

This tests the _workflow logic_ (state management, threshold
evaluation, alert generation, SRSPP routing) without the computational
cost of image processing. It is appropriate for integration testing
and for validating the communication path from detection to ground.

Example anomaly model for tailings dam monitoring:

```yaml
anomaly:
  type: displacement-ramp
  center: [-23.42, -43.10]  # lat, lon within AOI
  radius_px: 30
  onset: "2026-04-15T00:00:00Z"
  rate_mm_per_day: 0.5
  max_mm: 15.0
```

#### Synthetic Raster Injection

Pre-generated sensor images are loaded from disk and injected into the
workflow's collect phase when the satellite passes the AOI. The
workflow processes them through its full map/reduce pipeline.

- **SAR**: synthetic SLC (Single Look Complex) images with known phase
  patterns. A baseline master image is generated once; subsequent
  images add a displacement phase screen at the anomaly location.
  Atmospheric noise can be added as a random phase screen.
- **Thermal IR**: synthetic brightness temperature rasters. Background
  temperatures follow a diurnal model; anomaly pixels (fire ignition
  points) are injected at configurable locations and intensities.
- **Multispectral**: synthetic NDVI maps. Baseline values are set per
  land cover class; deforestation events reduce NDVI in a configurable
  polygon.
- **SAR backscatter**: for oil spill and flood use cases, synthetic
  backscatter images with dark-spot or water/non-water patterns
  injected at known locations.

Raster files are stored alongside the workflow definition and indexed
by pass number. The simulator selects the next file each time the
app's AOI check triggers a collection.

This tests the _full pipeline_ including the map phase's image
processing algorithms. It requires generating realistic-enough
synthetic data, but avoids the complexity of a physics-based sensor
simulator.

#### Real Data Replay

Archived satellite data is replayed through the workflow pipeline:

- **Sentinel-1** SAR (C-band, free, global) for InSAR, oil spill, flood,
  and ice use cases.
- **MODIS/VIIRS** thermal for wildfire detection.
- **Landsat/Sentinel-2** multispectral for deforestation.

Real data provides ground truth for validating detection algorithms
but requires downloading and pre-processing imagery for the specific
AOI. It also requires matching the simulation's orbital geometry to
the real satellite's revisit times, or abstracting away the timing
and replaying images in orbital-pass order regardless of actual
acquisition dates.

### Workflow App Integration

The workflow cFS app runs as a standard LeoDOS application:

1. **Startup**: registers workflows from a table loaded via CFE Table
   Services. Each workflow entry specifies the AOI, pipeline ID,
   thresholds, and sensor data source (parametric, synthetic raster, or
   replay).
2. **Collection trigger**: the app periodically reads its GPS position
   and checks against registered AOI bounding boxes. When the
   sub-satellite point enters an AOI, the app reads sensor data from
   the configured source.
3. **Processing**: the map and reduce phases execute onboard. For
   parametric mode, this is a threshold check on the injected anomaly
   descriptor. For raster and replay modes, this is the full image
   processing pipeline.
4. **Alert routing**: if anomalies are detected, the app sends an alert
   packet via SRSPP through the ISL router toward the nearest LOS
   node for ground downlink.
5. **State persistence**: baselines and workflow metadata are stored in
   the cFE Critical Data Store (CDS) for small data, or the onboard
   filesystem for large baselines (SAR master images).

### Tiered Fidelity for Workflows

Workflow simulation benefits from the tiered fidelity model:

- **Full-tier** nodes are assigned to satellites that pass over the AOI.
  These run the complete NOS3 stack with the workflow cFS app and
  sensor data injection. For a typical tailings dam workflow with a
  small AOI, 2--4 full-tier nodes suffice.
- **Lite-tier** nodes run cFS with the ISL router for realistic alert
  routing but do not execute workflow pipelines. They forward alert
  packets toward the ground.
- **Ghost-tier** nodes provide orbital positions for constellation
  topology. The network fabric forwards packets on their behalf.

This keeps resource usage manageable: a 100-satellite constellation
might use 3 full-tier nodes (over the AOI), 10 lite nodes (routing
corridor to ground), and 87 ghost nodes (topology).

### Network Fabric for Alert Routing

The network fabric applies physics-based constraints to alert
packets as they traverse the ISL mesh from the detecting satellite
to the ground station:

- **Line-of-sight gating**: alerts can only be forwarded when the
  next-hop satellite is not occluded by Earth. This tests the
  router's store-and-forward behaviour.
- **Propagation delay**: realistic light-speed delay between nodes
  (1--5 ms for typical LEO ISL distances).
- **Bandwidth constraints**: alert packets compete with routine
  telemetry for downlink bandwidth during ground passes.

This validates the end-to-end alert latency: time from anomaly
detection to ground receipt, including ISL hops, queuing delays,
and ground pass wait times.

### Per-Use-Case Simulation Details

| Use case | Recommended strategy | Notes |
|---|---|---|
| Tailings dam | Synthetic raster | Generate SLC pair with displacement phase screen. Master image stored as baseline. Inject increasing displacement over successive passes. |
| Wildfire | Parametric | Thermal hotspot injection is simple (coordinates + temperature). Full raster adds little value since the detection algorithm is a threshold. |
| Deforestation | Real data replay | Sentinel-2 NDVI time series over known deforestation events (e.g. Brazilian Amazon). Provides realistic seasonal variation for false-positive testing. |
| Oil spill | Synthetic raster | Generate SAR backscatter image with inserted dark spot of configurable shape and area. Background sea clutter from a statistical model. |
| Flood | Real data replay | Sentinel-1 SAR over documented flood events (e.g. 2024 Valencia floods). Pre-flood / post-flood image pair for change detection validation. |
| Volcanic | Synthetic raster | Same as tailings dam -- different AOI, larger displacement magnitudes (cm-scale). |
| Sea ice | Real data replay | Sentinel-1 SAR over Arctic corridors during known breakup events. Edge detection algorithms benefit from real ice texture. |
| Anti-poaching | Synthetic raster | Generate SAR backscatter with injected point targets (vehicles) on access roads. Vary time-of-day and location to test nighttime filtering and reserve boundary logic. |
