use crate::ffi;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("not enough memory")]
    NoMemory,
    #[error("invalid argument")]
    InvalidArgument,
    #[error("operation timed out")]
    Timeout,
    #[error("resource already in use")]
    ResourceInUse,
    #[error("operation not supported")]
    NotSupported,
    #[error("device or resource busy")]
    Busy,
    #[error("connection already in progress")]
    AlreadyInProgress,
    #[error("connection reset")]
    ConnectionReset,
    #[error("no more buffer space available")]
    NoBuffers,
    #[error("transmission failed")]
    TransmitFailed,
    #[error("error in driver layer")]
    DriverError,
    #[error("resource temporarily unavailable")]
    TryAgain,
    #[error("function not implemented")]
    NotImplemented,
    #[error("HMAC verification failed")]
    HmacFailed,
    #[error("CRC32 verification failed")]
    Crc32Failed,
    #[error("SFP protocol error")]
    SfpError,
    #[error("invalid MTU")]
    MtuError,
    #[error("null pointer returned")]
    NullPointer,
    #[error("unknown error ({0})")]
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

pub type Result<T> = core::result::Result<T, Error>;

pub(crate) fn check(code: i32) -> Result<()> {
    match Error::from_csp(code) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
