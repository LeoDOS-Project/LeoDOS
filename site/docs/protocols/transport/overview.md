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
fit within the MTU (Maximum Transfer Unit --- the largest packet
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

See the [CFDP page](cfdp) for the full description (727.0-B-5).

## Comparison

