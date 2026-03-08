//! NOS3 (NASA Operational Simulator for Small Sats) interfaces.
//!
//! Bus drivers (UART, I2C, SPI, CAN, GPIO, socket, torquer,
//! memory-mapped I/O) and simulated spacecraft component
//! drivers (sensors, actuators, radio, GPS, camera).

pub mod uart;
pub mod i2c;
pub mod spi;
pub mod gpio;
pub mod can;
pub mod socket;
pub mod trq;
pub mod mem;
pub mod components;

use crate::ffi;

// ── UART errors ──────────────────────────────────────────────

/// Errors from UART operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum UartError {
    /// Generic OS/driver error (`UART_ERROR`).
    #[error("UART: OS error")]
    OsError,
    /// File descriptor open error (`UART_FD_OPEN`).
    #[error("UART: file descriptor open error")]
    FdOpen,
    /// Unhandled error code.
    #[error("UART: unhandled error ({0})")]
    Unhandled(i32),
}

fn check_uart(rc: i32) -> Result<(), UartError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::UART_ERROR => Err(UartError::OsError),
        _ if rc == ffi::UART_FD_OPEN => Err(UartError::FdOpen),
        other => Err(UartError::Unhandled(other)),
    }
}

fn check_uart_count(rc: i32) -> Result<usize, UartError> {
    if rc >= 0 {
        Ok(rc as usize)
    } else {
        Err(match rc {
            _ if rc == ffi::UART_ERROR => UartError::OsError,
            _ if rc == ffi::UART_FD_OPEN => UartError::FdOpen,
            other => UartError::Unhandled(other),
        })
    }
}

// ── I2C errors ───────────────────────────────────────────────

/// Errors from I2C operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum I2cError {
    /// Generic OS/driver error (`I2C_ERROR`).
    #[error("I2C: OS error")]
    OsError,
    /// File descriptor open error (`I2C_FD_OPEN_ERR`).
    #[error("I2C: file descriptor open error")]
    FdOpen,
    /// Unhandled error code.
    #[error("I2C: unhandled error ({0})")]
    Unhandled(i32),
}

fn check_i2c(rc: i32) -> Result<(), I2cError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::I2C_ERROR => Err(I2cError::OsError),
        _ if rc == ffi::I2C_FD_OPEN_ERR => Err(I2cError::FdOpen),
        other => Err(I2cError::Unhandled(other)),
    }
}

// ── SPI errors ───────────────────────────────────────────────

/// Errors from SPI operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum SpiError {
    /// Generic OS/driver error (`SPI_ERROR`).
    #[error("SPI: OS error")]
    OsError,
    /// File open error (`SPI_ERR_FILE_OPEN`).
    #[error("SPI: file open error")]
    FileOpen,
    /// File handle error (`SPI_ERR_FILE_HANDLE`).
    #[error("SPI: file handle error")]
    FileHandle,
    /// File close error (`SPI_ERR_FILE_CLOSE`).
    #[error("SPI: file close error")]
    FileClose,
    /// Invalid SPI mode (`SPI_ERR_INVAL_MD`).
    #[error("SPI: invalid mode")]
    InvalidMode,
    /// IOC message error (`SPI_ERR_IOC_MSG`).
    #[error("SPI: IOC message error")]
    IocMsg,
    /// Write mode error (`SPI_ERR_WR_MODE`).
    #[error("SPI: write mode error")]
    WriteMode,
    /// Read mode error (`SPI_ERR_RD_MODE`).
    #[error("SPI: read mode error")]
    ReadMode,
    /// Write bits-per-word error (`SPI_ERR_WR_BPW`).
    #[error("SPI: write bits-per-word error")]
    WriteBpw,
    /// Read bits-per-word error (`SPI_ERR_RD_BPW`).
    #[error("SPI: read bits-per-word error")]
    ReadBpw,
    /// Write speed error (`SPI_ERR_WR_SD_HZ`).
    #[error("SPI: write speed error")]
    WriteSpeed,
    /// Read speed error (`SPI_ERR_RD_SD_HZ`).
    #[error("SPI: read speed error")]
    ReadSpeed,
    /// Mutex create error (`SPI_ERR_MUTEX_CREATE`).
    #[error("SPI: mutex create error")]
    MutexCreate,
    /// Unhandled error code.
    #[error("SPI: unhandled error ({0})")]
    Unhandled(i32),
}

fn check_spi(rc: i32) -> Result<(), SpiError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::SPI_ERROR => Err(SpiError::OsError),
        _ if rc == ffi::SPI_ERR_FILE_OPEN => Err(SpiError::FileOpen),
        _ if rc == ffi::SPI_ERR_FILE_HANDLE => Err(SpiError::FileHandle),
        _ if rc == ffi::SPI_ERR_FILE_CLOSE => Err(SpiError::FileClose),
        _ if rc == ffi::SPI_ERR_INVAL_MD => Err(SpiError::InvalidMode),
        _ if rc == ffi::SPI_ERR_IOC_MSG => Err(SpiError::IocMsg),
        _ if rc == ffi::SPI_ERR_WR_MODE => Err(SpiError::WriteMode),
        _ if rc == ffi::SPI_ERR_RD_MODE => Err(SpiError::ReadMode),
        _ if rc == ffi::SPI_ERR_WR_BPW => Err(SpiError::WriteBpw),
        _ if rc == ffi::SPI_ERR_RD_BPW => Err(SpiError::ReadBpw),
        _ if rc == ffi::SPI_ERR_WR_SD_HZ => Err(SpiError::WriteSpeed),
        _ if rc == ffi::SPI_ERR_RD_SD_HZ => Err(SpiError::ReadSpeed),
        _ if rc == ffi::SPI_ERR_MUTEX_CREATE => Err(SpiError::MutexCreate),
        other => Err(SpiError::Unhandled(other)),
    }
}

// ── GPIO errors ──────────────────────────────────────────────

/// Errors from GPIO operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum GpioError {
    /// Generic OS/driver error (`GPIO_ERROR`).
    #[error("GPIO: OS error")]
    OsError,
    /// File descriptor open error (`GPIO_FD_OPEN_ERR`).
    #[error("GPIO: file descriptor open error")]
    FdOpen,
    /// Write error (`GPIO_WRITE_ERR`).
    #[error("GPIO: write error")]
    Write,
    /// Read error (`GPIO_READ_ERR`).
    #[error("GPIO: read error")]
    Read,
    /// Unhandled error code.
    #[error("GPIO: unhandled error ({0})")]
    Unhandled(i32),
}

fn check_gpio(rc: i32) -> Result<(), GpioError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::GPIO_ERROR => Err(GpioError::OsError),
        _ if rc == ffi::GPIO_FD_OPEN_ERR => Err(GpioError::FdOpen),
        _ if rc == ffi::GPIO_WRITE_ERR => Err(GpioError::Write),
        _ if rc == ffi::GPIO_READ_ERR => Err(GpioError::Read),
        other => Err(GpioError::Unhandled(other)),
    }
}

// ── CAN errors ───────────────────────────────────────────────

/// Errors from CAN bus operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum CanError {
    /// Generic OS/driver error (`CAN_ERROR`).
    #[error("CAN: OS error")]
    OsError,
    /// Interface up error (`CAN_UP_ERR`).
    #[error("CAN: interface up error")]
    UpError,
    /// Interface down error (`CAN_DOWN_ERR`).
    #[error("CAN: interface down error")]
    DownError,
    /// Set modes error (`CAN_SET_MODES_ERR`).
    #[error("CAN: set modes error")]
    SetModes,
    /// Set bitrate error (`CAN_SET_BITRATE_ERR`).
    #[error("CAN: set bitrate error")]
    SetBitrate,
    /// Socket open error (`CAN_SOCK_OPEN_ERR`).
    #[error("CAN: socket open error")]
    SocketOpen,
    /// Socket flag set error (`CAN_SOCK_FLAGSET_ERR`).
    #[error("CAN: socket flag set error")]
    SocketFlagSet,
    /// Socket bind error (`CAN_SOCK_BIND_ERR`).
    #[error("CAN: socket bind error")]
    SocketBind,
    /// Write error (`CAN_WRITE_ERR`).
    #[error("CAN: write error")]
    Write,
    /// Read error (`CAN_READ_ERR`).
    #[error("CAN: read error")]
    Read,
    /// Read timeout (`CAN_READ_TIMEOUT_ERR`).
    #[error("CAN: read timeout")]
    ReadTimeout,
    /// Socket set option error (`CAN_SOCK_SETOPT_ERR`).
    #[error("CAN: socket set option error")]
    SocketSetOpt,
    /// Unhandled error code.
    #[error("CAN: unhandled error ({0})")]
    Unhandled(i32),
}

fn check_can(rc: i32) -> Result<(), CanError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::CAN_ERROR => Err(CanError::OsError),
        _ if rc == ffi::CAN_UP_ERR => Err(CanError::UpError),
        _ if rc == ffi::CAN_DOWN_ERR => Err(CanError::DownError),
        _ if rc == ffi::CAN_SET_MODES_ERR => Err(CanError::SetModes),
        _ if rc == ffi::CAN_SET_BITRATE_ERR => Err(CanError::SetBitrate),
        _ if rc == ffi::CAN_SOCK_OPEN_ERR => Err(CanError::SocketOpen),
        _ if rc == ffi::CAN_SOCK_FLAGSET_ERR => Err(CanError::SocketFlagSet),
        _ if rc == ffi::CAN_SOCK_BIND_ERR => Err(CanError::SocketBind),
        _ if rc == ffi::CAN_WRITE_ERR => Err(CanError::Write),
        _ if rc == ffi::CAN_READ_ERR => Err(CanError::Read),
        _ if rc == ffi::CAN_READ_TIMEOUT_ERR => Err(CanError::ReadTimeout),
        _ if rc == ffi::CAN_SOCK_SETOPT_ERR => Err(CanError::SocketSetOpt),
        other => Err(CanError::Unhandled(other)),
    }
}

// ── Socket errors ────────────────────────────────────────────

/// Errors from socket operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum SocketError {
    /// Generic OS/driver error (`SOCKET_ERROR`).
    #[error("Socket: OS error")]
    OsError,
    /// Socket create error (`SOCKET_CREATE_ERR`).
    #[error("Socket: create error")]
    Create,
    /// Socket bind error (`SOCKET_BIND_ERR`).
    #[error("Socket: bind error")]
    Bind,
    /// Socket listen error (`SOCKET_LISTEN_ERR`).
    #[error("Socket: listen error")]
    Listen,
    /// Socket accept error (`SOCKET_ACCEPT_ERR`).
    #[error("Socket: accept error")]
    Accept,
    /// Socket connect error (`SOCKET_CONNECT_ERR`).
    #[error("Socket: connect error")]
    Connect,
    /// Socket receive error (`SOCKET_RECV_ERR`).
    #[error("Socket: receive error")]
    Recv,
    /// Socket send error (`SOCKET_SEND_ERR`).
    #[error("Socket: send error")]
    Send,
    /// Socket close error (`SOCKET_CLOSE_ERR`).
    #[error("Socket: close error")]
    Close,
    /// Non-blocking operation would block (`SOCKET_TRY_AGAIN`).
    #[error("Socket: try again")]
    TryAgain,
    /// Unhandled error code.
    #[error("Socket: unhandled error ({0})")]
    Unhandled(i32),
}

fn check_socket(rc: i32) -> Result<(), SocketError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::SOCKET_ERROR => Err(SocketError::OsError),
        _ if rc == ffi::SOCKET_CREATE_ERR => Err(SocketError::Create),
        _ if rc == ffi::SOCKET_BIND_ERR => Err(SocketError::Bind),
        _ if rc == ffi::SOCKET_LISTEN_ERR => Err(SocketError::Listen),
        _ if rc == ffi::SOCKET_ACCEPT_ERR => Err(SocketError::Accept),
        _ if rc == ffi::SOCKET_CONNECT_ERR => Err(SocketError::Connect),
        _ if rc == ffi::SOCKET_RECV_ERR => Err(SocketError::Recv),
        _ if rc == ffi::SOCKET_SEND_ERR => Err(SocketError::Send),
        _ if rc == ffi::SOCKET_CLOSE_ERR => Err(SocketError::Close),
        _ if rc == ffi::SOCKET_TRY_AGAIN => Err(SocketError::TryAgain),
        other => Err(SocketError::Unhandled(other)),
    }
}

// ── Torquer errors ───────────────────────────────────────────

/// Errors from torquer operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum TrqError {
    /// Initialisation error (`TRQ_INIT_ERR`).
    #[error("Torquer: init error")]
    Init,
    /// Self-test error (`TRQ_SELFTEST_ERR`).
    #[error("Torquer: self-test error")]
    SelfTest,
    /// Connect error (`TRQ_CONNECT_ERR`).
    #[error("Torquer: connect error")]
    Connect,
    /// Invalid torquer number (`TRQ_NUM_ERR`).
    #[error("Torquer: invalid torquer number")]
    NumError,
    /// Time high value out of range (`TRQ_TIME_HIGH_VAL_ERR`).
    #[error("Torquer: time high value error")]
    TimeHighVal,
    /// Unhandled error code.
    #[error("Torquer: unhandled error ({0})")]
    Unhandled(i32),
}

fn check_trq(rc: i32) -> Result<(), TrqError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::TRQ_INIT_ERR => Err(TrqError::Init),
        _ if rc == ffi::TRQ_SELFTEST_ERR => Err(TrqError::SelfTest),
        _ if rc == ffi::TRQ_CONNECT_ERR => Err(TrqError::Connect),
        _ if rc == ffi::TRQ_NUM_ERR => Err(TrqError::NumError),
        _ if rc == ffi::TRQ_TIME_HIGH_VAL_ERR => Err(TrqError::TimeHighVal),
        other => Err(TrqError::Unhandled(other)),
    }
}

// ── Memory errors ────────────────────────────────────────────

/// Errors from device memory operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum MemError {
    /// Generic OS/driver error (`MEM_ERROR`).
    #[error("DevMem: OS error")]
    OsError,
    /// Unhandled error code.
    #[error("DevMem: unhandled error ({0})")]
    Unhandled(i32),
}

fn check_mem(rc: i32) -> Result<(), MemError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::MEM_ERROR => Err(MemError::OsError),
        other => Err(MemError::Unhandled(other)),
    }
}
