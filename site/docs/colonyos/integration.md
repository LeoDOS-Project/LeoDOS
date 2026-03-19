# ColonyOS Integration

ColonyOS is a meta-OS for distributed computing. It coordinates heterogeneous workers (executors) via a central server using a pull-based model: executors call `assign()` when ready, receive a process specification, execute it, and report the result. Every colony, executor, and user has an ECDSA key pair — all messages are signed and verified.

The challenge is integrating this with a satellite constellation where nodes are predictably unreachable.

## The Keepalive Problem

ColonyOS assumes executors are always reachable. If an executor stops sending keepalives, the server considers it dead and reassigns its work. This works for cloud workers and ground servers, but satellites go in and out of ground contact every ~95 minutes. A satellite executor would be considered dead every time it loses line of sight — which is the normal state, not a failure.

## Design Options

Three architectures were evaluated:

### A1: Direct Communication

Every satellite is a ColonyOS executor. Each holds a blocking `assign()` connection routed through the LOS gateway.

**Problems:** hundreds of connections through one gateway. Every phase transition bounces through the ground. Satellites constantly considered dead due to LOS cycling.

### A2: Satellite Coordinator

One satellite is the ColonyOS executor. It receives jobs from the server and coordinates other satellites over ISL. Data flows directly between satellites.

**Improvement:** ColonyOS sees one job in, one result out. No per-phase ground round-trips.

**Remaining problem:** the coordinator satellite also goes in and out of LOS. Same keepalive issue.

### A3: Ground Coordinator (Recommended)

A ground process is the ColonyOS executor. It is always reachable — no keepalive problem. It receives jobs, computes the plan using orbital predictions, uploads assignments to the constellation via the current LOS satellite, and collects results when they arrive.

**Advantages:**
- No keepalive issue — ground executor is always reachable
- Satellites execute autonomously over ISL
- ColonyOS sees a single, always-available executor
- No changes to ColonyOS required

## Job Flow

### Ground-Originated

1. User submits a [SpaceCoMP](/spacecomp/overview) job to ColonyOS (AOI, algorithm, parameters).
2. Ground coordinator pulls the job via `assign()`.
3. Coordinator computes the plan: which satellites are collectors, mappers, reducer (using orbital predictions).
4. Coordinator uploads assignments to the constellation via the current LOS satellite.
5. Satellites execute autonomously over ISL. Data flows directly between satellites via [SRSPP](/protocols/transport/srspp).
6. Result routes back through whichever satellite has LOS at completion — Earth rotates during the job, so the originating LOS satellite may no longer be visible.
7. Ground coordinator reports the result to ColonyOS.

### Satellite-Originated

1. A satellite detects an anomaly in sensor data (e.g., displacement threshold exceeded in a [workflow](/spacecomp/use-cases/overview)).
2. It routes a job request through the ISL mesh to the LOS satellite.
3. LOS satellite relays the request to the ground coordinator.
4. Ground coordinator submits to ColonyOS, pulls it back, and orchestrates as above.

For time-critical cases, a satellite could coordinate directly in orbit — planning and assigning roles without involving the ground. This eliminates the ground round-trip but cannot leverage ColonyOS for orchestration.

## Design Comparison

|  | A1: Direct | A2: Sat Coord. | A3: Gnd Coord. | B: Sat Server |
|---|---|---|---|---|
| Keepalive issue | Yes | Yes | **No** | Reduced |
| Ground round-trips | Every phase | Job in/out | Job in/out | None |
| ISL data flow | No | Yes | Yes | Yes |
| ColonyOS changes | None | None | None | Reimplementation |
| Practical today | Poor fit | Poor fit | **Yes** | No |

## Future: Design B — Satellite Server

ColonyOS server running on a satellite. Executors (other satellites) communicate entirely over ISL — no ground round-trips for orchestration. The keepalive problem is reduced because satellites can always reach the server over ISL.

**Challenges:** ColonyOS is implemented in Go and requires a full OS environment. Running it on a satellite would require an embedded reimplementation (`no_std`, [cFS](/cfs/overview)). A single satellite server is also a single point of failure.
