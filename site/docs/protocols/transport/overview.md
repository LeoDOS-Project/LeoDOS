# Overview

The transport layer provides end-to-end reliable delivery. Below
this layer, reliability is per-hop (COP-1) and packets can still
be lost at intermediate routers. The transport layer guarantees
that data arrives at the destination application complete, in
order, and without duplicates, regardless of how many hops the path
traverses.

## SRSPP

The Simple Reliable Space Packet Protocol provides reliable
delivery of variable-size messages. It is designed for the
request-response patterns typical of command/telemetry and
distributed computing (SpaceCoMP).

On the send side, SRSPP segments a message into Space Packets that
fit within the MTU (Maximum Transfer Unit — the largest packet
the network layer can carry in one piece), assigns each a sequence
number, and transmits them through the network layer. It maintains
a retransmission buffer and a timer for each outstanding packet.

On the receive side, SRSPP collects incoming packets, reorders
them by sequence number, detects gaps, and reassembles the
original message. It sends acknowledgments back to the sender:
cumulative ACKs confirm all packets up to a sequence number, and
selective ACKs identify specific packets received beyond a gap.

When the sender receives an ACK, it removes acknowledged packets
from the retransmission buffer. When a timer expires without
acknowledgment, it retransmits the unacknowledged packet. The
retransmission timeout adapts to observed round-trip times.

Three receiver backends trade off memory and performance:

- **Fast**: Optimized for throughput. Uses more memory to allow
  rapid insertion and retrieval.
- **Lite**: Minimizes memory usage. Suitable for resource-constrained
  satellites.
- **Packed**: Uses compact in-place storage for a balance between
  the two.

SRSPP has platform-specific async APIs for both the Tokio runtime
(used in ground stations and simulation) and the cFS runtime (used
on flight software).

See the [detailed SRSPP page](srspp) for the full protocol
specification.

## CFDP

The CCSDS File Delivery Protocol transfers files between spacecraft and ground stations. Unlike SRSPP which delivers messages, CFDP delivers named files — it handles segmentation, reassembly, and optionally retransmission of missing segments. CFDP operates in two classes: Class 1 (unreliable, no feedback) and Class 2 (reliable, with NAK-based retransmission).

See the [CFDP page](cfdp) for the full description (727.0-B-5).

## BP

The Bundle Protocol (BPv7, RFC 9171) is a store-and-forward protocol designed for Delay/Disruption Tolerant Networking (DTN). Unlike SRSPP and CFDP which assume a connected path exists at transmission time, BP bundles can be stored at intermediate nodes and forwarded when a link becomes available — making it suitable for networks where end-to-end connectivity is intermittent or unpredictable.

Each bundle carries a source and destination Endpoint ID (EID), a lifetime, and optional extension blocks (hop count, bundle age, custody transfer). Bundles are encoded in CBOR and delivered through Convergence Layer Adapters (CLAs) that map onto the underlying transport — UDP, TCP, or the LeoDOS ISL stack.

LeoDOS provides Rust bindings to NASA's bplib (BPv7 implementation) through the `leodos-libcfs` crate with the `bp` feature flag. The bindings cover endpoint IDs, application channels (send/receive ADUs), convergence layer contacts (bundle ingress/egress), and engine lifecycle management.

See the [BP page](bp) for the full protocol description.

## Comparison

