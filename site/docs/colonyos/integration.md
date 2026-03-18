# ColonyOS Integration

ColonyOS is an external job orchestration framework that coordinates computation across heterogeneous workers — ground servers, edge devices, and satellites. LeoDOS integrates with ColonyOS as a bridge between ground-initiated workflows and the onboard [SpaceCoMP](/spacecomp/overview) execution model.

## Role in LeoDOS

SpaceCoMP handles the onboard side: task allocation, data collection, map/reduce processing, and alert routing across the constellation. But the workflow must be *initiated* — someone needs to define the job, select the target satellites, and schedule execution. ColonyOS fills this role:

1. **Job submission** — a ground operator or automated system submits a job to ColonyOS with a geographic area of interest, sensor requirements, and processing pipeline.
2. **Colony assignment** — ColonyOS determines which "colony" (a group of satellites with the right orbital coverage) can execute the job and assigns it.
3. **Uplink** — the job definition is translated into a SpaceCoMP workflow descriptor and uplinked to the constellation via [CFDP](/protocols/transport/cfdp).
4. **Execution** — the constellation executes the workflow autonomously using SpaceCoMP's [Collect-Map-Reduce model](/spacecomp/overview).
5. **Result collection** — alert packets and processed results are downlinked and reported back to ColonyOS as completed tasks.

## Why an External Orchestrator

The constellation cannot orchestrate itself for ground-initiated jobs. Satellites have intermittent ground contact (5–15 minutes per pass), limited visibility into the full constellation state, and no persistent connection to ground systems. ColonyOS provides:

- **Persistent job queue** — jobs survive ground station outages and are uplinked on the next available pass.
- **Multi-colony coordination** — a single job can span multiple ground stations and multiple satellite groups if the AOI is large.
- **Heterogeneous workers** — some processing (baseline generation, historical comparison, result archiving) happens on ground servers. ColonyOS coordinates the ground and space portions of a workflow as a single job.

## Workflow Lifecycle

```
Ground operator
  → ColonyOS (job queue, scheduling)
    → Ground station (uplink via CFDP)
      → Constellation (SpaceCoMP execution)
        → Downlink (alert packets via SRSPP)
          → ColonyOS (result collection)
            → Ground operator (dashboard, archive)
```

The onboard portion is autonomous — once the workflow descriptor is loaded into the satellite's [Table Services](/cfs/cfe/tbl), the cFS app executes it on every qualifying pass without further ground interaction. ColonyOS only re-engages when results arrive or when the ground wants to update or cancel the workflow.
