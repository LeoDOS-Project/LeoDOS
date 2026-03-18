# UART

Universal Asynchronous Receiver/Transmitter. A serial bus that
sends bytes one bit at a time over a wire. The standard interface
between the flight computer and the radio. The FSW writes bytes to
the UART transmit register; the radio modulates them onto the
carrier. In the receive direction, the radio demodulates the
incoming signal and presents bytes on the UART receive register.
