# Delay-Tolerant Extensions

Standard reliable transport protocols (like TCP) assume the receiver is reachable. When a packet isn't acknowledged, they retransmit — and after enough failures, they give up and tear down the connection. In a satellite constellation, the receiver is regularly unreachable: ground stations go below the horizon every orbit, and ISL links can be interrupted by the ascending/descending seam.

SRSPP handles this without a separate DTN layer by making the retransmission timeout aware of orbital geometry.

## The Problem

With a fixed retransmission timeout, a sender targeting a ground station that is currently below the horizon will:

1. Send a packet
2. Wait for the timeout (e.g., 500 ms)
3. Retransmit
4. Wait again
5. Retransmit again
6. After 3 attempts, declare the packet lost

All three retransmit attempts were wasted — the ground station was never reachable. The sender burned through its retry budget during a predictable, normal orbital gap. When the ground station comes back into view, the sender has already given up.

## Contact Schedules

The orbit-aware RTO policy solves this with a *contact schedule* — a list of time windows during which the receiver is expected to be reachable.

Each window records:
- **Station ID** — which ground station
- **Start time** — when the satellite enters line of sight (in mission-elapsed seconds)
- **End time** — when the satellite exits line of sight

The schedule is stored in a fixed-size buffer (`heapless::Vec`) suitable for flight processors with [bounded memory](/cfs/mission/memory). Windows are kept in chronological order.

Because satellite orbits are [deterministic](/background/orbits), contact windows can be predicted hours or days in advance from orbital parameters. The schedule can be uploaded as a [cFE table](/cfs/cfe/tbl) and updated from the ground.

## How It Works

When the sender needs a retransmission timeout, it queries the RTO policy with the current time:

- **Inside a contact window** — the receiver is expected to be reachable. Use a short timeout (the normal ISL RTO, e.g., 500 ms). If the packet isn't acknowledged, it was genuinely lost and should be retransmitted quickly.
- **Between contact windows** — the receiver is expected to be unreachable. Set the timeout to the time remaining until the next contact window, plus a configurable margin. The sender holds the packet instead of retransmitting into a gap. When the window opens, the timer expires and the packet is retransmitted when the link is actually available.
- **No future windows in the schedule** — fall back to the short ISL timeout. The schedule may not cover the entire mission; once it runs out, the sender behaves like a fixed-RTO policy.

## Example

A satellite at mission time 250 seconds has a contact schedule:

- Window 1: station Kiruna, 100–200 s (past)
- Window 2: station Svalbard, 500–600 s (future)

The sender needs an RTO at time 250:
- Not inside any window (250 is between 200 and 500)
- Next window starts at 500
- Time until next window: 250 seconds
- RTO = 250 × 1000 + margin ticks

The sender holds the packet for ~250 seconds. When mission time reaches 500 and the Svalbard window opens, the timer expires and the packet is retransmitted over the now-available link.

## Comparison with BP

The [Bundle Protocol](/protocols/transport/bp) solves intermittent connectivity by storing bundles at intermediate nodes (store-and-forward). SRSPP's orbit-aware RTO solves it differently: the sender holds the packet and retransmits at the right time, without involving intermediate nodes.

- BP is more general — it handles multi-hop store-and-forward where intermediate nodes may also be intermittently connected.
- SRSPP's approach is simpler — no custody transfer, no bundle storage at intermediate nodes. It works when the sender can predict when the receiver will be reachable.

For LEO constellations where contact windows are predictable, the orbit-aware RTO is sufficient. For deep space or unpredictable connectivity, BP's store-and-forward model is needed.
