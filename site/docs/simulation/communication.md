# Communication

The simulation models two communication paths: the ground link (satellite to ground station) and inter-satellite links (satellite to satellite). Both use the generic radio simulator, which replaces the physical RF transceiver with UDP sockets.

## Ground Link

The ground link connects a satellite to a ground station. In simulation, this is a pair of UDP sockets — one for uplink (ground to satellite), one for downlink (satellite to ground). The flight software sends and receives frames through the same radio driver API it uses in flight; the simulator routes them over localhost instead of RF.

## Inter-Satellite Links

Inter-satellite links (ISL) connect neighboring satellites in the [2D torus topology](/spacecomp/constellation). Each satellite has a proximity radio that communicates with its neighbors over UDP sockets. The [ISL router](/protocols/network/routing) forwards packets across multiple hops, and [SRSPP](/protocols/transport/srspp) provides reliable delivery over the multi-hop path.

## Hardware Bus Emulation

NOS Engine replaces physical hardware buses with TCP connections. The hwlib layer in cFS provides the same API (uart_read, spi_write, i2c_transfer) regardless of whether the underlying transport is a real hardware register or a NOS Engine TCP socket. This is what makes the flight software binary identical in simulation and flight — the bus abstraction is the boundary.

Supported bus types:
- **UART** — serial communication (radio, thrusters, GPS)
- **SPI** — high-speed peripherals (cameras, sensors)
- **I2C** — low-speed peripherals (arducam, EPS)
- **CAN** — controller area network

## What Is Not Simulated

The communication simulation provides connectivity but does not model the physical RF channel:

- **Propagation delay** — packets arrive instantly over UDP. Real ISL links have 1–5 ms propagation delay depending on distance.
- **Line-of-sight gating** — contact windows between satellites are not enforced by the radio simulator. In flight, a satellite can only communicate with a neighbor when it is not occluded by Earth.
- **Bandwidth constraints** — full throughput is assumed. Real RF links have limited data rates that constrain how much data can be transmitted during a contact window.
- **Path loss and link margin** — signal strength is not modeled. Real links degrade with distance and atmospheric effects.
- **Interference and noise** — the channel is clean. Real RF links are subject to interference from other transmitters and thermal noise.

These gaps mean the simulation validates the protocol logic (framing, routing, reliability) but not the link budget. RF performance must be validated separately with link analysis tools or hardware-in-the-loop testing.
