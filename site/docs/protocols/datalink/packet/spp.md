# SPP

Implementation of CCSDS 133.0-B-2 Space Packet Protocol. SPP provides the
network layer for space communications, encapsulating application data in
fixed-format packets with routing information.

## Primary Header

All space packets begin with a 6-byte primary header:

| Field               | Bits  | Description                              |
|---------------------|-------|------------------------------------------|
| Packet Version      | 3     | Always 0 (Version 1)                     |
| Packet Type         | 1     | 0 = Telemetry, 1 = Telecommand           |
| Secondary Hdr Flag  | 1     | 0 = Absent, 1 = Present                  |
| APID                | 11    | Application Process ID (0-2047)          |
| Sequence Flags      | 2     | Segmentation status                      |
| Sequence Count      | 14    | Rolling counter (0-16383)                |
| Packet Data Length  | 16    | Data field length minus 1                |

## Sequence Flags

| Value | Name         | Meaning                        |
|-------|--------------|--------------------------------|
| 0b00  | Continuation | Middle segment of a message    |
| 0b01  | First        | First segment of a message     |
| 0b10  | Last         | Final segment of a message     |
| 0b11  | Unsegmented  | Complete message in one packet |

## APID

The Application Process Identifier routes packets to specific applications:

- Range: 0-2047 (11 bits)
- APID 2047 (0x7FF) is reserved for idle packets
- APIDs are mission-specific

## Data Field

- Minimum: 1 byte
- Maximum: 65536 bytes
- Contains optional secondary header + user data

## cFE Message ID

The cFE software bus derives a 16-bit Message ID from the SPP header:

```
Bits 0-10:  APID (11 bits)
Bit 11:     Type (0=TM, 1=TC)
Bit 12:     SB Flag (always 1)
Bits 13-15: Reserved (0)
```

## Wire Format

All multi-byte fields use network byte order (big-endian).
