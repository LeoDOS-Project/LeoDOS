# TM

Used for the downlink (satellite to ground). A TM frame has a
fixed length configured at link setup time. The header contains:

- **Spacecraft ID (SCID)**: identifies the satellite.
- **Virtual Channel ID (VCID)**: multiplexes multiple data streams
  over a single physical link. For example, real-time telemetry
  and stored science data can share a downlink on separate virtual
  channels.
- **Master Channel Frame Counter**: increments for every frame on
  the physical link, across all virtual channels.
- **Virtual Channel Frame Counter**: increments for every frame on
  this virtual channel specifically. COP-1 uses this counter to
  detect lost frames.
- **First Header Pointer**: the byte offset within the data field
  where the first Space Packet begins. This allows the receiver
  to find packet boundaries even when packets span multiple frames.

The fixed frame length simplifies the coding layer (RS and ASM
operate on fixed-size blocks) and enables the receiver to achieve
frame synchronization without delimiters.
