# ColonyOS Integration

ColonyOS is a meta-OS for distributed computing. It coordinates heterogeneous workers (executors) via a central server using a pull-based model: executors call `assign()` when ready, receive a process specification, execute it, and report the result. Every colony, executor, and user has an ECDSA key pair — all messages are signed and verified.

The challenge is integrating this with a satellite constellation where nodes are predictably unreachable. ColonyOS assumes executors are always reachable — if an executor stops sending keepalives, the server considers it dead and reassigns its work. Satellites go in and out of ground contact every ~95 minutes. A satellite executor would be considered dead every time it loses line of sight, which is the normal state, not a failure.

## Ground Coordinator

The solution: a ground process acts as the ColonyOS executor. It is always reachable — no keepalive problem. ColonyOS sees a single, always-available executor and requires no modifications. Satellites execute autonomously over ISL, outside ColonyOS's awareness.

### Ground-Originated Jobs

1. User submits a [SpaceCoMP](/spacecomp/overview) job to ColonyOS (AOI, algorithm, parameters).
2. Ground coordinator pulls the job via `assign()`.
3. Coordinator computes the [plan](/spacecomp/job-lifecycle): which satellites are [collectors, mappers, reducer](/spacecomp/roles) (using orbital predictions).
4. Coordinator uploads assignments to the constellation via the current LOS satellite.
5. Satellites execute autonomously over ISL. Data flows directly between satellites via [SRSPP](/protocols/transport/srspp).
6. Result routes back through whichever satellite has LOS at completion — Earth rotates during the job, so the originating LOS satellite may no longer be visible.
7. Ground coordinator reports the result to ColonyOS.

### Satellite-Originated Jobs

1. A satellite detects an anomaly in sensor data (e.g., displacement threshold exceeded in a [workflow](/spacecomp/use-cases/overview)).
2. It routes a job request through the ISL mesh to the LOS satellite.
3. LOS satellite relays the request to the ground coordinator.
4. Ground coordinator submits to ColonyOS, pulls it back, and orchestrates as above.

For time-critical cases, a satellite could coordinate directly in orbit — planning and assigning roles without involving the ground. This eliminates the ground round-trip but cannot leverage ColonyOS for orchestration.

## Alternative Designs Considered

### Direct Communication

Every satellite is a ColonyOS executor, each holding a blocking `assign()` connection routed through the LOS gateway. This is a poor fit: hundreds of connections through one gateway, every phase transition bounces through the ground, and satellites are constantly considered dead due to LOS cycling.

### Satellite Coordinator

One satellite is the ColonyOS executor. It receives jobs from the server and coordinates other satellites over ISL. Data flows directly between satellites, and ColonyOS sees one job in, one result out. But the coordinator satellite also goes in and out of LOS — the same keepalive problem remains.

### Satellite Server (Future)

ColonyOS server running on a satellite. Executors (other satellites) communicate entirely over ISL — no ground round-trips for orchestration. The keepalive problem is reduced because satellites can always reach the server over ISL. However, ColonyOS is implemented in Go and requires a full OS environment. Running it on a satellite would require an embedded reimplementation (`no_std`, [cFS](/cfs/overview)). A single satellite server is also a single point of failure.

## Comparison

|  | Direct | Sat Coordinator | **Ground Coordinator** | Sat Server |
|---|---|---|---|---|
| Keepalive issue | Yes | Yes | **No** | Reduced |
| Ground round-trips | Every phase | Job in/out | Job in/out | None |
| ISL data flow | No | Yes | Yes | Yes |
| ColonyOS changes | None | None | **None** | Reimplementation |
| Practical today | No | No | **Yes** | No |
