# Job Lifecycle

A SpaceCoMP job flows through four phases: submission, planning, execution, and result delivery. The entire lifecycle can complete within a single orbital pass or span multiple passes depending on the area of interest and network conditions.

## Submission

A ground station submits a job by sending a `SubmitJob` message to the [coordinator](roles). The job definition is a 41-byte wire-format structure containing:

- **Geographic AOI** — upper-left and lower-right coordinates (latitude/longitude)
- **Data volume** — bytes per collector task
- **Processing factors** — `map_processing_factor` and `reduce_processing_factor` (relative computational cost)
- **Reduction factors** — `map_reduction_factor` and `reduce_reduction_factor` (compression ratios between phases)
- **Flags** — ascending-only restriction (avoids the [seam](constellation)), assignment solver choice ([Hungarian](task-allocation/hungarian) or [LAPJV](task-allocation/lapjv))

The job is uplinked via the communication stack to the coordinator satellite — typically the line-of-sight gateway or a satellite near the AOI.

## Planning

The coordinator converts the geographic AOI to a grid-space bounding box using the [Walker Delta projection](constellation) and produces a plan:

1. **Collector selection** — all satellites whose nadir falls within the grid AOI. If the ascending-only flag is set, only satellites in ascending-phase planes are selected.
2. **Mapper selection** — satellites within or near the AOI grid, chosen for spatial distribution.
3. **Assignment** — the coordinator solves a bipartite matching problem to pair collectors with mappers. The cost of each assignment is computed by a pluggable cost model.
4. **Reducer placement** — a single satellite chosen by the placement strategy (LineOfSight or CenterOfAoi).
5. **Cost estimation** — the total job cost in microseconds, used for scheduling and ground reporting.

### Cost Models

Two cost models are available:

**SpaceCoMP cost model** — physics-based, accounting for:
- Processing time at each node
- Per-hop overhead on ISL links (default: 100 μs)
- Transmission time via Shannon channel capacity: propagation delay (distance/c) plus transfer time (data volume / bandwidth × log₂(1 + SNR))
- SNR computed from Friis transmission equation: transmit power, antenna gains, free-space path loss, and noise power (Boltzmann-Nyquist)
- Default parameters tuned for optical ISLs (10 GHz bandwidth, 1550 nm wavelength)

**Manhattan cost model** — topology-based, using grid distance times a fixed hop cost. Used for testing and baseline comparisons.

### Total Job Cost

The end-to-end cost combines mapping and reduction:

$$
C_{\text{total}} = C_{\text{map}} + C_{\text{reduce}}
$$

where $C_{\text{map}}$ sums the processing, hop, and transmission costs for all collector-to-mapper paths, and $C_{\text{reduce}}$ sums the aggregation cost from all mappers to the reducer plus the reducer-to-LOS delivery cost.

## Execution

After planning, the coordinator broadcasts assignment messages:

1. Each **collector** receives its assigned mapper address and partition ID. It begins data collection and transmits its partition to the mapper via [SRSPP](/protocols/transport/srspp).
2. Each **mapper** receives the reducer address and the number of collectors it should expect. It waits for all collector data, processes it, and forwards the intermediate result to the reducer.
3. The **reducer** receives the LOS gateway address and the number of mappers it should expect. It waits for all mapper results, aggregates them, and sends the final output toward the ground.

Each role signals completion with a `PhaseDone` message to the coordinator. The coordinator tracks progress and can detect stalled phases.

## Result Delivery

The reducer sends the final result as a `JobResult` message, which is routed through the ISL mesh to the line-of-sight gateway and downlinked to the ground station. For small results (alert packets, anomaly summaries), SRSPP provides reliable delivery. For larger results, [CFDP](/protocols/transport/cfdp) file transfer is used.

The result arrives at the ground via the same communication stack used for all telemetry — it flows through the [Software Bus](/cfs/cfe/sb), the transfer frame layer, and the RF downlink. [ColonyOS](/colonyos/integration) can collect and archive results as completed tasks in the external job queue.
