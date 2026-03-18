# Routing

SpaceCoMP uses a distance-minimizing shortest-path algorithm that exploits the physical geometry of the [Walker Delta constellation](constellation) to reduce total ISL distance traversed while maintaining the same hop count as standard Manhattan routing on the torus.

## Manhattan Routing

The baseline routing algorithm on a 2D torus computes the shortest path in grid hops — the Manhattan distance between source and destination, accounting for wraparound on both axes. Each hop moves one step in either the orbital-plane axis (east/west) or the satellite axis (north/south). This minimizes hop count but ignores the fact that ISL distances vary with orbital position.

## Distance-Minimizing Routing

The distance-minimizing algorithm produces the same hop count as Manhattan routing but chooses the *order* of hops to minimize the total physical distance traversed. The key insight: cross-plane ISL distances vary by ~40% depending on the satellite's position along its orbit.

- Near the **equator**, orbital planes are far apart — cross-plane hops are expensive.
- Near the **poles**, orbital planes converge — cross-plane hops are cheap.
- Intra-plane distances are constant regardless of position.

The algorithm computes a **cross-plane factor** at each routing decision:

$$
f(\theta) = \sqrt{\cos^2(\theta) + \cos^2(i) \cdot \sin^2(\theta)}
$$

where $\theta$ is the true anomaly (satellite's position along the orbit) and $i$ is the orbital inclination.

At each hop, the router checks whether the cross-plane factor would increase or decrease by moving to the next plane. If it would increase (moving toward the equator where cross-plane links are longer), the router prefers an intra-plane hop first — moving along the satellite axis toward a position where cross-plane hops are cheaper. If the factor is decreasing (moving toward the poles), the router takes the cross-plane hop now while it is cheap.

## Results

The algorithm achieves 8–21% reduction in total ISL distance compared to naive Manhattan routing, with zero hop count overhead. The savings come entirely from reordering the same set of hops to exploit orbital geometry. This translates directly to reduced propagation delay and improved link margins (SNR decreases with distance).

## Integration with SpaceCoMP

The [job planner](job-lifecycle) uses the distance-minimizing algorithm in its cost model to estimate the total communication cost of a job plan. The [ISL router](/protocols/network/routing) uses it at runtime to forward packets between SpaceCoMP [roles](roles). Because the algorithm produces the same hop count as Manhattan routing, it does not affect the assignment problem — it only improves the physical cost of each assigned path.
