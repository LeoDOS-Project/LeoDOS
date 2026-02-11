#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WasiError {
    Success = 0,
    InvalidHandle = -1,
    InvalidArgument = -2,
    NoCapacity = -3,
    IoError = -4,
    Timeout = -5,
    NotFound = -6,
    PermissionDenied = -7,
    AlreadyExists = -8,
    EndOfFile = -12,
    EndOfDirectory = -13,
}

impl WasiError {
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => WasiError::Success,
            -1 => WasiError::InvalidHandle,
            -2 => WasiError::InvalidArgument,
            -3 => WasiError::NoCapacity,
            -4 => WasiError::IoError,
            -5 => WasiError::Timeout,
            -6 => WasiError::NotFound,
            -7 => WasiError::PermissionDenied,
            -8 => WasiError::AlreadyExists,
            -12 => WasiError::EndOfFile,
            -13 => WasiError::EndOfDirectory,
            _ => WasiError::IoError,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            WasiError::Success => "success",
            WasiError::InvalidHandle => "invalid handle",
            WasiError::InvalidArgument => "invalid argument",
            WasiError::NoCapacity => "no capacity",
            WasiError::IoError => "I/O error",
            WasiError::Timeout => "timeout",
            WasiError::NotFound => "not found",
            WasiError::PermissionDenied => "permission denied",
            WasiError::AlreadyExists => "already exists",
            WasiError::EndOfFile => "end of file",
            WasiError::EndOfDirectory => "end of directory",
        }
    }
}

pub type Result<T> = core::result::Result<T, WasiError>;

pub fn check(code: i32) -> Result<()> {
    if code >= 0 {
        Ok(())
    } else {
        Err(WasiError::from_code(code))
    }
}

pub fn check_with_value(code: i32) -> Result<i32> {
    if code >= 0 {
        Ok(code)
    } else {
        Err(WasiError::from_code(code))
    }
}
