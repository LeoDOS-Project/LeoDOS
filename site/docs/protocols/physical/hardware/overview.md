# Overview

The flight computer communicates with peripherals (radios, sensors,
actuators) over hardware buses. NOS3 hwlib provides drivers for
seven bus types, all of which are link-time substituted in
simulation: the same FSW binary runs on real hardware or in NOS3,
with only the linked driver library changing.

- [UART](uart) — serial interface to the radio
- [SPI](spi) — synchronous full-duplex bus for sensors
- [I2C](i2c) — two-wire bus for low-rate sensors
- [CAN](can) — differential bus for subsystem communication
- [UDP/TCP](udp-tcp) — network sockets for simulation and ground links

## GPIO

General Purpose I/O. Direct pin-level control for simple signals
(enable lines, status flags, interrupt triggers).
