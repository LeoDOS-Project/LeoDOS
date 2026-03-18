# Tiered Fidelity

A LEO constellation may have 100+ satellites, but most simulations only need high fidelity for a few. A wildfire detection workflow involves 2–4 satellites over the area of interest and a routing corridor to ground — the other 90+ satellites only need to exist as network topology. Running full NOS3 instances for all of them wastes resources without improving test coverage.

LeoDOS defines three simulation tiers:

## Full Tier

A full-tier node runs the complete stack: 42 orbital propagation, all NOS3 hardware simulators, cFS with the full LeoDOS app suite, and sensor data injection. It is indistinguishable from a real satellite in terms of software behavior.

Full-tier nodes are assigned to satellites that are directly involved in the test scenario — those that pass over the area of interest, execute workflow pipelines, or are endpoints of the communication path being tested. For a typical workflow test, 2–4 full-tier nodes suffice.

## Lite Tier

A lite-tier node runs cFS with the [ISL router](/protocols/network/routing) and [Software Bus](/cfs/cfe/sb), but no hardware simulators and no sensor payloads. Its orbital position is computed analytically (no 42 instance). It can route packets, forward alerts, and participate in the [gossip protocol](/protocols/network/gossip), but it does not generate or process sensor data.

Lite nodes are used for the routing corridor between a full-tier satellite and the ground station. They test realistic multi-hop routing behavior (store-and-forward, link availability based on orbital geometry) without the overhead of a full NOS3 instance per node.

## Ghost Tier

A ghost-tier node has no running software at all. It exists only as an entry in the constellation topology table — an orbital position that other nodes know about for routing calculations. The network fabric forwards packets on behalf of ghost nodes using a simplified model: fixed latency, fixed link availability based on orbital geometry.

Ghost nodes fill out the constellation to its full size so that routing algorithms operate on a realistic topology. Without them, the torus would have gaps that change the routing paths.

## Typical Allocation

For a 100-satellite constellation testing a wildfire detection workflow:

| Tier | Count | Purpose |
|---|---|---|
| Full | 3 | Satellites over the AOI (run workflow + sensor sim) |
| Lite | 10 | Routing corridor from AOI to ground station |
| Ghost | 87 | Fill out the 2D torus topology |

The total resource cost is dominated by the 3 full-tier nodes. Adding more ghost nodes is nearly free — they are just table entries.
