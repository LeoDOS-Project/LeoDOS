use crate::ffi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    NoMemory,
    InvalidArgument,
    Timeout,
    ResourceInUse,
    NotSupported,
    Busy,
    AlreadyInProgress,
    ConnectionReset,
    NoBuffers,
    TransmitFailed,
    DriverError,
    TryAgain,
    NotImplemented,
    HmacFailed,
    Crc32Failed,
    SfpError,
    MtuError,
    NullPointer,
    Unknown(i32),
}

impl Error {
    pub(crate) fn from_csp(code: i32) -> Option<Self> {
        if code >= 0 {
            return None;
        }
        Some(match code {
            x if x == ffi::CSP_ERR_NOMEM => Error::NoMemory,
            x if x == ffi::CSP_ERR_INVAL => Error::InvalidArgument,
            x if x == ffi::CSP_ERR_TIMEDOUT => Error::Timeout,
            x if x == ffi::CSP_ERR_USED => Error::ResourceInUse,
            x if x == ffi::CSP_ERR_NOTSUP => Error::NotSupported,
            x if x == ffi::CSP_ERR_BUSY => Error::Busy,
            x if x == ffi::CSP_ERR_ALREADY => Error::AlreadyInProgress,
            x if x == ffi::CSP_ERR_RESET => Error::ConnectionReset,
            x if x == ffi::CSP_ERR_NOBUFS => Error::NoBuffers,
            x if x == ffi::CSP_ERR_TX => Error::TransmitFailed,
            x if x == ffi::CSP_ERR_DRIVER => Error::DriverError,
            x if x == ffi::CSP_ERR_AGAIN => Error::TryAgain,
            x if x == ffi::CSP_ERR_NOSYS => Error::NotImplemented,
            x if x == ffi::CSP_ERR_HMAC => Error::HmacFailed,
            x if x == ffi::CSP_ERR_CRC32 => Error::Crc32Failed,
            x if x == ffi::CSP_ERR_SFP => Error::SfpError,
            x if x == ffi::CSP_ERR_MTU => Error::MtuError,
            _ => Error::Unknown(code),
        })
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::NoMemory => write!(f, "not enough memory"),
            Error::InvalidArgument => write!(f, "invalid argument"),
            Error::Timeout => write!(f, "operation timed out"),
            Error::ResourceInUse => write!(f, "resource already in use"),
            Error::NotSupported => write!(f, "operation not supported"),
            Error::Busy => write!(f, "device or resource busy"),
            Error::AlreadyInProgress => write!(f, "connection already in progress"),
            Error::ConnectionReset => write!(f, "connection reset"),
            Error::NoBuffers => write!(f, "no more buffer space available"),
            Error::TransmitFailed => write!(f, "transmission failed"),
            Error::DriverError => write!(f, "error in driver layer"),
            Error::TryAgain => write!(f, "resource temporarily unavailable"),
            Error::NotImplemented => write!(f, "function not implemented"),
            Error::HmacFailed => write!(f, "HMAC verification failed"),
            Error::Crc32Failed => write!(f, "CRC32 verification failed"),
            Error::SfpError => write!(f, "SFP protocol error"),
            Error::MtuError => write!(f, "invalid MTU"),
            Error::NullPointer => write!(f, "null pointer returned"),
            Error::Unknown(code) => write!(f, "unknown error ({})", code),
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn check(code: i32) -> Result<()> {
    match Error::from_csp(code) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
