# ISL Gossip Protocol

The ISL Gossip Protocol provides broadcast message dissemination across a
satellite constellation. It floods messages to all satellites within a
specified service area while preventing duplicate delivery.

## Use Cases

- Distributing configuration updates
- Propagating consensus/blockchain data
- Broadcasting telemetry to ground-visible satellites

## Packet Structure

```text
+---------------------------------+-----------+
| Field Name                      | Size      |
+---------------------------------+-----------+
| Primary Header (SPP)            | 6 bytes   |
| Secondary Header (cFE)          | 2 bytes   |
+---------------------------------+-----------+
| Originator Address              | 2 bytes   |
| From Address                    | 2 bytes   |
| Service Area Min                | 1 byte    |
| Service Area Max                | 1 byte    |
| Epoch                           | 2 bytes   |
| Action Code                     | 1 byte    |
+---------------------------------+-----------+
| Payload                         | variable  |
+---------------------------------+-----------+
```

### Header Fields

| Field            | Type       | Description                                 |
|------------------|------------|---------------------------------------------|
| originator       | RawAddress | Address of the node that created this gossip |
| from_address     | RawAddress | Address of the immediate sender (for routing) |
| service_area_min | u8         | Minimum satellite_id in target service area |
| service_area_max | u8         | Maximum satellite_id in target service area |
| epoch            | U16        | Unique sequence number for duplicate detection |
| action_code      | u8         | Application-specific action identifier      |

## Address Fields

Two addresses serve different purposes:

- **originator**: The node that created the gossip. Stays constant as the
  packet propagates. Used for application-level identification.

- **from_address**: The immediate sender. Updated at each hop. Used to avoid
  echoing the packet back to the sender.

## Service Area

The service area defines which satellites should receive and process the gossip.
It's specified as a range of satellite IDs within each orbit:

```
service_area_min = 3
service_area_max = 7

Satellites 3, 4, 5, 6, 7 in each orbit will process the gossip.
```

### Wraparound

When `service_area_min > service_area_max`, the range wraps around:

```
service_area_min = 8
service_area_max = 2
constellation_size = 10

Satellites 8, 9, 0, 1, 2 are in the service area.
```

This handles the case where the ground-visible region crosses the orbit boundary.

## Duplicate Detection

Each gossip has a unique `epoch` identifier. The receiver maintains a cache of
recently seen epochs:

```rust
const EPOCH_CACHE_SIZE: usize = 128;

fn is_duplicate(&mut self, epoch: Epoch) -> bool {
    if self.epoch_cache.contains(&epoch) {
        return true;
    }
    self.epoch_cache[self.cache_index] = epoch;
    self.cache_index = (self.cache_index + 1) % EPOCH_CACHE_SIZE;
    false
}
```

The cache uses a circular buffer - old entries are overwritten when the cache
is full.

## Forwarding Logic

When a gossip packet arrives:

1. Check if epoch is in the duplicate cache
2. If duplicate, drop the packet
3. Otherwise:
   - Add epoch to cache
   - Deliver payload to application
   - Forward to eligible neighbors

### Neighbor Eligibility

A neighbor receives the forwarded gossip if:
1. It's not the sender (`to_address != from_address`)
2. It's within the service area (`satellite_id` in `[min, max]`)

```rust
for direction in [North, South, East, West] {
    let neighbor = torus.neighbor(my_position, direction);
    if neighbor != from_address
       && neighbor.is_in_service_area(min, max) {
        forward(direction, packet);
    }
}
```

## Gossip Handler

The `GossipHandler` processes incoming gossip packets:

```rust
pub struct GossipHandler<F> {
    epoch_cache: [Epoch; EPOCH_CACHE_SIZE],
    epoch_cache_index: u8,
    torus: Torus,
    my_address: Address,
    app_logic: F,  // Application callback
}
```

### Processing Flow

```
recv gossip packet
    |
    v
is_duplicate(epoch)?
    |
    +-- yes --> drop
    |
    +-- no --> app_logic(packet)
               |
               v
           forward_gossip()
               |
               v
           send to eligible neighbors
```

## Configuration

The gossip handler requires:
- Own address (orbit_id, satellite_id)
- Torus topology (for neighbor calculation)
- Application callback for payload processing
