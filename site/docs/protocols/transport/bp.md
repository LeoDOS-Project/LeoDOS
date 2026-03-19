# BP

The Bundle Protocol version 7 (BPv7, RFC 9171) is a store-and-forward protocol for Delay/Disruption Tolerant Networking (DTN). It is designed for networks where end-to-end connectivity cannot be guaranteed — nodes may be disconnected for hours or days, and bundles must survive in storage until a forwarding opportunity arises.

## Why BP

SRSPP and CFDP assume that a path exists between sender and receiver at the time of transmission. In a LEO constellation, this is usually true — the ISL mesh provides continuous connectivity between satellites. But ground contact is intermittent, and in some orbital configurations the entire constellation may be temporarily unreachable from all ground stations.

BP addresses this by making store-and-forward a first-class operation. A bundle is not dropped when the next hop is unavailable — it is stored locally and forwarded when the link comes up. This is the protocol's native behavior, not an error recovery mechanism.

## How It Works

A sending application submits an Application Data Unit (ADU) to the BP engine through a channel. The engine wraps the ADU in a bundle with:

- **Source and destination Endpoint IDs (EIDs)** — using the IPN URI scheme (e.g., `ipn:42.1` for node 42, service 1)
- **Lifetime** — how long the bundle remains valid before being discarded
- **CRC** — optional integrity check (CRC-16 or CRC-32C)
- **Extension blocks** — hop count, bundle age, previous node, custody transfer

The bundle is encoded in CBOR and passed to a Convergence Layer Adapter (CLA) for transmission. The CLA maps bundle delivery onto the underlying transport — UDP, TCP, or the LeoDOS ISL stack.

At each intermediate node, the BP engine receives the bundle from the CLA, checks its validity and lifetime, looks up the destination EID in the routing table, and either delivers it locally (if this node matches the destination) or forwards it through the appropriate contact.

## Endpoint IDs

BPv7 uses URI-based endpoint identifiers. LeoDOS uses the IPN scheme:

- **2-digit IPN** — `ipn:node.service` (e.g., `ipn:42.1`)
- **3-digit IPN** — `ipn:allocator.node.service` for large constellations with organizational hierarchy

EID patterns support wildcard matching for routing — a contact can be configured to forward all bundles destined for a range of nodes.

## Convergence Layer Adapters

A CLA adapts the BP engine to a specific transport:

| CLA Type | Transport | Use case |
|---|---|---|
| UDP | Unreliable datagrams | Low-latency ISL forwarding |
| TCP | Reliable stream | Ground station uplink/downlink |
| LTP | Licklider Transmission Protocol | Deep space links with very high delay |
| EPP | Encapsulation Packet Protocol | CCSDS integration |

## Custody Transfer

BP supports optional custody transfer — an intermediate node can accept custody of a bundle, taking responsibility for its delivery. The sender is released from retransmission duty once custody is acknowledged. This is useful when the sender has limited storage and wants to offload bundles to a node closer to the destination.

## Bundle Storage

Bundles awaiting forwarding are persisted in a storage backend (SQLite in bplib). The storage subsystem handles batch insertion, garbage collection of expired bundles, and prioritized egress for contacts and channels.

## LeoDOS Integration

LeoDOS provides Rust bindings to NASA's bplib through the `leodos-libcfs` crate (`bp` feature). The bindings cover:

- **Endpoint IDs** — `Eid::ipn(node, service)` with pattern matching
- **Channels** — `channel::send()` / `channel::recv()` for application data
- **Contacts** — `contact::setup()` / `contact::start()` / `contact::ingress()` / `contact::egress()` for CLA integration
- **Engine lifecycle** — memory pool, queue manager, worker threads, storage
