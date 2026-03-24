//! Hardware bus drivers.

/// Errors from bus operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum BusError {
    /// A uart error.
    #[error("UART error: {0}")]
    Uart(#[from] uart::UartError),
    /// An I2C error.
    #[error("I2C error: {0}")]
    I2c(#[from] i2c::I2cError),
    /// An SPI error.
    #[error("SPI error: {0}")]
    Spi(#[from] spi::SpiError),
    /// A GPIO error.
    #[error("GPIO error: {0}")]
    Gpio(#[from] gpio::GpioError),
    /// A CAN error.
    #[error("CAN error: {0}")]
    Can(#[from] can::CanError),
    /// A socket error.
    #[error("Socket error: {0}")]
    Socket(#[from] socket::SocketError),
    /// A torquer error.
    #[error("Trq error: {0}")]
    Trq(#[from] trq::TrqError),
    /// A memory-mapped I/O error.
    #[error("Memory-mapped I/O error: {0}")]
    Mem(#[from] mem::MemError),
}

/// UART (serial) bus.
pub mod uart;
/// I2C (two-wire) bus.
pub mod i2c;
/// SPI (synchronous serial) bus.
pub mod spi;
/// GPIO (general-purpose I/O) pins.
pub mod gpio;
/// CAN (controller area network) bus.
pub mod can;
/// Network sockets (TCP/UDP).
pub mod socket;
/// Magnetorquer PWM control.
pub mod trq;
/// Memory-mapped I/O.
pub mod mem;
