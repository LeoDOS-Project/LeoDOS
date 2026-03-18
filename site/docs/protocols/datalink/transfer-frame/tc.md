# TC

Used for the uplink (ground to satellite). TC frames are
variable-length (up to a configured maximum). The header contains
SCID, VCID, frame sequence number, and two flags:

- **Bypass flag**: when set, the frame bypasses the COP-1 sequence
  check and is delivered immediately. Used for emergency commands
  when the COP-1 state may be out of sync.
- **Control flag**: distinguishes command frames (carrying Space
  Packets) from control frames (carrying COP-1 directives).

TC frames are typically much shorter than TM frames because
commands are small and uplink bandwidth is limited.
