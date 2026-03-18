# Overview

CCSDS SPP (133.0-B-2) defines the packet format used by all higher
layers. Each Space Packet has a 6-byte primary header containing:

- **APID** (Application Process Identifier): an 11-bit value that
  identifies which application or service the packet belongs to.
  The receiver uses the APID to dispatch incoming packets to the
  correct handler.
- **Sequence Count**: a 14-bit counter that increments per packet
  per APID. Used by the transport layer to detect gaps and
  reorder.
- **Sequence Flags**: indicate whether the packet is unsegmented, or
  the first/continuation/last segment of a larger payload.
- **Data Length**: the number of bytes in the data field.

When a payload is too large for a single Space Packet (constrained
by the maximum frame size), the segmenter splits it into multiple
packets with appropriate sequence flags. The reassembler on the
receiving side collects the segments and reconstructs the original
payload.

The Encapsulation Packet Protocol (CCSDS 133.1-B-3) extends SPP
with encapsulation packets that can wrap non-CCSDS data or serve
as idle fill when the link has no data to send but must maintain
frame synchronization.

Space Packets are extracted from transfer frames by the link
reader, which parses the data field using the First Header Pointer
and packet length fields to find packet boundaries even when
packets span multiple frames.

- [SPP](spp) — Space Packet header format
