# Roles

SpaceCoMP distributes computation across four roles: coordinator, collector, mapper, and reducer. Each role is a mode of operation within the same cFS app — any satellite can take on any role depending on the job assignment.

## Coordinator

The coordinator receives job requests from the ground station (via the line-of-sight gateway node), plans the computation, and assigns roles to participating satellites. Planning involves:

1. Converting the geographic area of interest to [grid coordinates](constellation).
2. Identifying collector satellites — those whose nadir falls within the AOI.
3. Selecting mapper satellites — distributed within or near the AOI grid region.
4. Choosing a reducer location using one of two placement strategies:
   - **LineOfSight** — reducer at the LOS gateway, minimizing reducer-to-ground hop cost.
   - **CenterOfAoi** — reducer at the AOI center, minimizing mapper-to-reducer aggregation cost.
5. Solving the [assignment problem](task-allocation/hungarian) to optimally pair collectors with mappers.
6. Broadcasting assignment messages to all participants.

The coordinator does not process data itself — it orchestrates the pipeline and waits for phase-completion signals from each participant.

## Collector

Collectors are satellites positioned over the area of interest. They gather raw sensor data from onboard instruments during the collection window (the period when the satellite's ground track intersects the AOI). Each collector is assigned to a specific mapper by the coordinator.

After collection, the collector sends its data to its assigned mapper via [SRSPP](/protocols/transport/srspp) through the ISL [routing](/protocols/network/routing) layer. Data is transmitted as fixed-size record chunks.

## Mapper

Mappers receive raw data from one or more collectors and perform the computation-intensive processing step — feature extraction, anomaly detection, compression, or transformation depending on the job's pipeline. Each mapper processes its partition independently and in parallel with other mappers.

After processing, the mapper forwards its intermediate results to the reducer. The reduction in data volume between collector→mapper and mapper→reducer is controlled by the job's `map_reduction_factor` — for example, a wildfire detection mapper might reduce a 200 MB thermal image to a few KB anomaly mask.

## Reducer

A single satellite is designated as the reducer. It aggregates results from all mappers into the final output — merging anomaly masks, computing global statistics, or assembling the consolidated result. The reducer then routes the final result to the line-of-sight gateway for downlink to the ground station.

The reducer's location is chosen by the coordinator's placement strategy. Placing the reducer at the LOS gateway minimizes the final hop to ground; placing it at the AOI center minimizes the aggregate distance from all mappers.

## Message Flow

```
Ground Station
  │
  ▼
Coordinator ──── AssignCollector ────► Collector(s)
     │                                     │
     ├─── AssignMapper ──────────────► Mapper(s)
     │                                     │
     ├─── AssignReducer ─────────────► Reducer
     │                                     │
     │            DataChunk ◄──────────────┘ (collector → mapper)
     │            DataChunk ◄──────────────── (mapper → reducer)
     │                                     │
     │◄───────── PhaseDone ────────────────┘ (each role signals completion)
     │◄───────── JobResult ────────────────── (reducer → coordinator → ground)
```

Each arrow is a SpaceCoMP protocol message routed over SRSPP. The opcodes are:

| OpCode | Direction | Purpose |
|---|---|---|
| `SubmitJob` | Ground → Coordinator | Job definition with AOI, parameters, solver choice |
| `AssignCollector` | Coordinator → Collector | Mapper address + partition ID |
| `AssignMapper` | Coordinator → Mapper | Reducer address + expected collector count |
| `AssignReducer` | Coordinator → Reducer | LOS address + expected mapper count |
| `DataChunk` | Between phases | Fixed-size record payload |
| `PhaseDone` | Any role → Coordinator | Phase completion signal |
| `JobResult` | Reducer → Coordinator | Final aggregated result |

## Data Schema

Data flowing between roles is defined by a generic `Schema` trait. Each schema type specifies a fixed-size record layout, a partition key (for mapping collectors to reducers), and zero-copy serialization. The I/O layer connects roles through async `Source` and `Sink` traits — a collector's sink writes to the network, and a mapper's source reads from it, with SRSPP providing reliable delivery between them.
