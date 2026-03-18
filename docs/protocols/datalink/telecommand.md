# Telecommand Packets (cFE TC)

cFE Telecommand packets carry commands from ground to spacecraft. They extend
the SPP primary header with a secondary header containing command identification
and integrity checking.

## Packet Structure

```text
+---------------------------+-----------+
| Field                     | Size      |
+---------------------------+-----------+
| Primary Header (SPP)      | 6 bytes   |
| Secondary Header          | 2 bytes   |
| Payload                   | variable  |
+---------------------------+-----------+
| Minimum Total             | 8 bytes   |
+---------------------------+-----------+
```

## Secondary Header

```text
+---------------------------+-----------+
| Field                     | Size      |
+---------------------------+-----------+
| Function Code             | 1 byte    |
| Checksum                  | 1 byte    |
+---------------------------+-----------+
| Total                     | 2 bytes   |
+---------------------------+-----------+
```

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

```rust
pub fn checksum_u8(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc ^ b)
}

pub fn validate_checksum_u8(data: &[u8]) -> bool {
    checksum_u8(data) == 0
}
```

## Rust Structures

```rust
#[repr(C, packed)]
pub struct TelecommandSecondaryHeader {
    function_code: u8,   // 1 byte
    checksum: u8,        // 1 byte
}

pub struct Telecommand {
    primary: PrimaryHeader,                 // 6 bytes
    secondary: TelecommandSecondaryHeader,  // 2 bytes
    payload: [u8],                          // variable
}
```

## Constraints

- Maximum payload: 65534 bytes
- Function code is application-specific (0-255)

## Building a Telecommand Packet

```rust
let tc = Telecommand::builder()
    .buffer(&mut buf)
    .apid(apid)
    .sequence_count(seq)
    .function_code(cmd)
    .payload_len(data.len())
    .build()?;

// Checksum is calculated automatically
```

## Parsing a Telecommand Packet

```rust
let tc = Telecommand::parse(&buf)?;

if !tc.validate_cfe_checksum() {
    return Err(ChecksumError);
}

let code = tc.secondary.function_code;
let payload = tc.payload();
```
