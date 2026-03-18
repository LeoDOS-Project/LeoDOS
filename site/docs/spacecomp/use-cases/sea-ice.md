# Sea Ice

Arctic shipping lanes need advance warning of ice breakup events.

**Sensor:** SAR.

**Pipeline:**
- _Collect:_ SAR strip over monitored Arctic corridor.
- _Map:_ edge detection on ice boundaries. Compare against previous
  pass to detect fracture propagation.
- _Reduce:_ if fracture rate exceeds threshold or new leads (open water
  channels) appear in shipping lanes, generate navigation alert.
