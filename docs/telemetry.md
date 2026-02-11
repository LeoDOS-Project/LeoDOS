# Telemetry Packets (cFE TM)

cFE Telemetry packets carry data from spacecraft to ground. They extend the
SPP primary header with a secondary header containing timestamp information.

## Packet Structure

```text
+---------------------------+-----------+
| Field                     | Size      |
+---------------------------+-----------+
| Primary Header (SPP)      | 6 bytes   |
| Secondary Header          | 10 bytes  |
| Payload                   | variable  |
+---------------------------+-----------+
| Minimum Total             | 16 bytes  |
+---------------------------+-----------+
```

## Secondary Header

```text
+---------------------------+-----------+
| Field                     | Size      |
+---------------------------+-----------+
| Time (CDS format)         | 8 bytes   |
| Spare                     | 2 bytes   |
+---------------------------+-----------+
| Total                     | 10 bytes  |
+---------------------------+-----------+
```

| Field | Size    | Description                              |
|-------|---------|------------------------------------------|
| Time  | 8 bytes | CCSDS Day Segmented time (48 bits used)  |
| Spare | 2 bytes | Padding for alignment                    |

## Time Format

The time field uses CCSDS Day Segmented (CDS) format:
- 48 bits of the 64-bit field are active
- Bitmask: `0x0000_FFFF_FFFF_FFFF`
- Stored in network byte order

## Fixed Header Values

Telemetry packets always have:
- Packet Type = 0 (Telemetry)
- Secondary Header Flag = 1 (Present)
- Sequence Flags = 0b11 (Unsegmented)

## Rust Structures

```rust
#[repr(C, packed)]
pub struct TelemetrySecondaryHeader {
    time: U64,           // 8 bytes (48 bits active)
    spare: [u8; 2],      // 2 bytes padding
}

pub struct Telemetry {
    primary: PrimaryHeader,               // 6 bytes
    secondary: TelemetrySecondaryHeader,  // 10 bytes
    payload: [u8],                        // variable
}
```

## Constraints

- Maximum payload: 65534 bytes
- Time value must fit in 48 bits

## Building a Telemetry Packet

```rust
let tm = Telemetry::builder()
    .buffer(&mut buf)
    .apid(apid)
    .sequence_count(seq)
    .time(timestamp)
    .payload_len(data.len())
    .build()?;
```
