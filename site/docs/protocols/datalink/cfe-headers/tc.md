# TC

cFE Telecommand packets carry commands from ground to spacecraft. They extend
the SPP primary header with a secondary header containing command identification
and integrity checking.

## Packet Structure

| Field | Size |
|-------|------|
| Primary Header (SPP) | 6 bytes |
| Secondary Header | 2 bytes |
| Payload | variable |
| **Minimum Total** | **8 bytes** |

## Secondary Header

| Field | Size |
|-------|------|
| Function Code | 1 byte |
| Checksum | 1 byte |
| **Total** | **2 bytes** |

| Field         | Size   | Description                           |
|---------------|--------|---------------------------------------|
| Function Code | 1 byte | Command identifier within the APID    |
| Checksum      | 1 byte | 8-bit XOR checksum of entire packet   |

## Fixed Header Values

Telecommand packets always have:
- Packet Type = 1 (Telecommand)
- Secondary Header Flag = 1 (Present)
- Sequence Flags = 0b11 (Unsegmented)

## Checksum Algorithm

The cFE uses an 8-bit XOR checksum:

1. Set checksum field to 0
2. XOR all bytes of the packet together
3. Store result in checksum field

Validation: XOR of all bytes (including checksum) equals 0.

## Constraints

- Maximum payload: 65534 bytes
- Function code is application-specific (0-255)

