# Routing

The ISL (Inter-Satellite Link) Routing Protocol provides point-to-point message
delivery across a satellite constellation organized as a 2D torus topology.

## Topology

Satellites are arranged in a grid where:
- Each satellite has an `(orbit_id, satellite_id)` address
- Orbits wrap around (orbit 0 is adjacent to orbit N-1)
- Satellites within an orbit wrap around (sat 0 is adjacent to sat M-1)

This forms a 2D torus with four neighbors per satellite:
- **North**: Previous orbit (orbit_id - 1)
- **South**: Next orbit (orbit_id + 1)
- **East**: Next satellite in orbit (satellite_id + 1)
- **West**: Previous satellite in orbit (satellite_id - 1)

## Packet Structure

```text
+----------------------------+-----------+
| Field Name                 | Size      |
+----------------------------+-----------+
| Primary Header (SPP)       | 6 bytes   |
| Secondary Header (cFE)     | 2 bytes   |
+----------------------------+-----------+
| Target Address             | 2 bytes   |
| Message ID                 | 1 byte    |
| Action Code                | 1 byte    |
+----------------------------+-----------+
| Payload                    | variable  |
+----------------------------+-----------+
```

### Header Fields

| Field        | Type       | Description                                    |
|--------------|------------|------------------------------------------------|
| target       | RawAddress | Destination address (ground, satellite, or service area) |
| message_id   | u8         | Request/response correlation ID (0 for async)  |
| action_code  | u8         | Application-specific action identifier         |

## Address Encoding

The `RawAddress` wire format (2 bytes):

| ground_or_orbit | station_or_sat | Meaning                          |
|-----------------|----------------|----------------------------------|
| 0               | N              | Ground station N                 |
| K (1-255)       | 0              | Service area for orbit K-1       |
| K (1-255)       | M (1-255)      | Satellite (orbit=K-1, sat=M)     |

## Routing Algorithm

The router determines the next hop based on the shortest path through the torus:

1. Parse the target address from the packet header
2. Convert source and destination to torus points
3. Calculate Manhattan distance in each direction (accounting for wraparound)
4. Choose the direction that minimizes total distance
5. Forward to the appropriate neighbor link

### Direction Selection

```
if target == self:
    deliver to Local
else:
    calculate shortest path on torus
    forward to North, South, East, or West
```

## Router Operation

The `Router` struct manages six interfaces:
- **north, south, east, west**: Inter-satellite links
- **ground**: Earth communication link
- **local**: Application interface

### Main Loop

```
loop {
    packet = recv from any interface
    target = parse packet header
    direction = next_hop(target)
    forward packet to direction
}
```

## Error Handling

| Condition              | Behavior                              |
|------------------------|---------------------------------------|
| Invalid packet format  | Drop silently                         |
| Link send failure      | Log error, continue                   |
| Unknown target         | Route based on torus algorithm        |

## Configuration

The router requires:
- Own address (orbit_id, satellite_id)
- Constellation dimensions (max_orb, max_sat)
- Routing algorithm implementation
