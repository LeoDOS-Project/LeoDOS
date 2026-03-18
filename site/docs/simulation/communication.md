# Communication

The simulation replaces physical hardware buses and RF transceivers with software transports (UDP sockets and NOS Engine TCP connections), allowing satellites to communicate on a single development machine.

## Supported

### Hardware Buses

The simulation models the onboard buses that connect the flight computer to its peripherals. Each bus type is emulated by NOS Engine, which replaces the electrical signals with TCP messages while preserving the same byte-level protocol.

- **[UART](/protocols/physical/hardware/uart)** — asynchronous serial interface. Used for the radio transceiver (ground and inter-satellite links), GPS receiver, and thrusters. UART is the primary bus for devices that exchange variable-length messages at moderate data rates.
- **[SPI](/protocols/physical/hardware/spi)** — synchronous full-duplex bus with chip-select lines. Used for the thermal camera and high-speed sensor readout. SPI transfers data in both directions simultaneously — the flight computer clocks out a command while clocking in sensor data in the same transaction. Higher throughput than UART, used where frame-rate data transfer is needed.
- **[I2C](/protocols/physical/hardware/i2c)** — two-wire bus (clock + data) with device addressing. Used for the Arducam control interface, EPS telemetry, and other low-rate peripherals. Multiple devices share the same two wires, each responding to its own address. Simpler wiring than SPI but lower throughput.
- **[CAN](/protocols/physical/hardware/can)** — differential bus for subsystem communication. A multi-master protocol where any node can transmit, with built-in arbitration and error detection. Used for inter-subsystem messaging where reliability and noise immunity matter more than raw throughput.

### RF Links

The radio transceiver is connected to the flight computer over UART. The simulation replaces the RF side of the radio with UDP sockets:

- **Ground link** — a satellite communicates with a ground station when it has line of sight. The simulation models both uplink (commands from ground) and downlink (telemetry to ground) as separate UDP socket pairs.
- **Inter-satellite link** — communication between neighboring satellites in the constellation. In the real system, these would be optical or RF links. The simulation models them as UDP connections between satellite instances.

## Not Yet Supported

- **Propagation delay** — packets arrive instantly over UDP. Real inter-satellite links have 1–5 ms delay depending on distance.
- **RF channel effects** — signal-to-noise ratio, path loss, link margin degradation with distance, interference, and thermal noise are not modeled. The simulated channel is error-free.
- **Modulation and demodulation** — the [physical layer](/protocols/physical/overview) (BPSK, QPSK, etc.) is bypassed. In flight, the radio performs modulation on transmit and demodulation on receive; in simulation, raw frames pass directly over UDP.
- **Optical link physics** — for optical inter-satellite links, pointing accuracy, beam divergence, and atmospheric scintillation are not modeled.
