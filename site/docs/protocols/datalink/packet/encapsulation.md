# Encapsulation Packet

Implementation of CCSDS 133.1-B-3 Encapsulation Packet Protocol. Encapsulation
packets carry non-CCSDS protocol data (e.g. IP datagrams) over CCSDS space
links. They share the same packet layer as Space Packets but use a different
header format, distinguished by a Packet Version Number of 7 (binary `111`)
instead of 0.

## Header

The mandatory header is 2 bytes. A variable-length packet length field
follows, sized by the Length of Length field.

| Field                | Bits | Description                                   |
|----------------------|------|-----------------------------------------------|
| Packet Version       | 3    | Always `111` (7)                              |
| Protocol ID          | 4    | Identifies the encapsulated protocol          |
| Length of Length      | 2    | Number of bytes for the packet length field   |
| User Defined         | 4    | Mission-specific                              |
| Protocol ID Ext      | 4    | Extends Protocol ID when Protocol ID = `0010` |
| CCSDS Defined        | 1    | Reserved for CCSDS use                        |

## Protocol ID

The 4-bit Protocol ID identifies what is encapsulated:

| Value  | Name          | Description                          |
|--------|---------------|--------------------------------------|
| `0000` | Idle          | Idle packet (all-zeros payload)      |
| `0001` | IPE           | Internet Protocol Extension          |
| `0010` | CCSDS Defined | Type specified by extension field    |
| `0111` | User Defined  | Mission-specific protocol            |
| `1000` | IPv4          | Internet Protocol version 4          |
| `1001` | IPv6          | Internet Protocol version 6          |

When Protocol ID is `0010` (CCSDS Defined), the 4-bit Protocol ID
Extension field selects the specific CCSDS protocol.

## Packet Length

The Length of Length field determines how many bytes encode the
packet length:

| Length of Length | Bytes | Maximum packet size |
|-----------------|-------|---------------------|
| `00`            | 0     | Implicit/undefined  |
| `01`            | 1     | 255                 |
| `10`            | 2     | 65 535              |
| `11`            | 4     | 4 294 967 295       |

The total header size is 2 bytes (mandatory) plus the packet length
bytes (0, 1, 2, or 4).

## Comparison with SPP

Both [SPP](/protocols/datalink/packet/spp) and Encapsulation Packets
live in the same packet layer and can be multiplexed on the same
virtual channel. They are distinguished by the Packet Version Number
in the first 3 bits: 0 for SPP, 7 for Encapsulation. This lets a
receiver demultiplex them without any additional framing.
