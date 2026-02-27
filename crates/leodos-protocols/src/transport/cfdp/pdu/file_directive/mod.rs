use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

use crate::transport::cfdp::CfdpError;

/// Acknowledgment (ACK) PDU.
pub mod ack;
/// End-of-File (EOF) PDU.
pub mod eof;
/// Finished PDU.
pub mod finished;
/// Keep Alive PDU (small and large file variants).
pub mod keepalive;
/// Metadata PDU.
pub mod metadata;
/// Negative Acknowledgment (NAK) PDU (small and large file variants).
pub mod nak;
/// Prompt PDU.
pub mod prompt;

/// A zero-copy representation of the start of any File Directive's data field.
/// It provides the directive code, which can then be used to parse the `rest`
/// of the data field into a more specific PDU type (EofPdu, FinishedPdu, etc.).
///
/// ```text
/// +------------------------------------+----------------+
/// | Field Name                         | Size           |
/// +------------------------------------+----------------+
/// | Directive Code                     | 8 bits         |
/// | `rest` (contents depend on code)   | Variable       |
/// +------------------------------------+----------------+
/// ```
#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct FileDirectivePdu {
    directive_code: u8,
    rest: [u8],
}

/// Identifies the type of File Directive PDU (Table 5-4).
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DirectiveCode {
    /// End-of-File directive.
    Eof = 0x04,
    /// Finished directive.
    Finished = 0x05,
    /// Acknowledgment directive.
    Ack = 0x06,
    /// Metadata directive.
    Metadata = 0x07,
    /// Negative Acknowledgment directive.
    Nak = 0x08,
    /// Prompt directive.
    Prompt = 0x09,
    /// Keep Alive directive.
    KeepAlive = 0x0C,
}

impl DirectiveCode {
    /// Returns the size in bytes of the DirectiveCode field.
    pub fn size() -> usize {
        1
    }
}

impl TryFrom<u8> for DirectiveCode {
    type Error = CfdpError;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        let val = match val {
            0x04 => DirectiveCode::Eof,
            0x05 => DirectiveCode::Finished,
            0x06 => DirectiveCode::Ack,
            0x07 => DirectiveCode::Metadata,
            0x08 => DirectiveCode::Nak,
            0x09 => DirectiveCode::Prompt,
            0x0C => DirectiveCode::KeepAlive,
            _ => return Err(CfdpError::Custom("Invalid DirectiveCode value")),
        };
        Ok(val)
    }
}

impl FileDirectivePdu {
    /// Returns the DirectiveCode enum variant for this PDU.
    pub fn directive_code(&self) -> Result<DirectiveCode, CfdpError> {
        self.directive_code.try_into()
    }

    /// Sets the directive code for this PDU.
    pub fn set_directive_code(&mut self, code: DirectiveCode) {
        self.directive_code = code as u8;
    }

    /// Get the raw rest field as a byte slice.
    pub fn rest(&self) -> &[u8] {
        &self.rest
    }
}

/// Represents the Condition Code reported in `EOF`, `Finished`, and `ACK` PDUs.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
#[repr(u8)]
pub enum ConditionCode {
    /// No error was detected.
    #[default]
    NoError = 0,
    /// The acknowledgment limit was reached without receiving an expected ACK.
    AckLimitReached = 1,
    /// The keep-alive limit was reached without receiving any PDU for the transaction.
    KeepAliveLimitReached = 2,
    /// An invalid transmission mode was specified.
    InvalidTransmissionMode = 3,
    /// The filestore operation was rejected by the receiver.
    FilestoreRejection = 4,
    /// The file checksum did not match the expected value.
    FileChecksumFailure = 5,
    /// The file size did not match the expected value.
    FileSizeError = 6,
    /// The NAK limit was reached without satisfying all missing data requests.
    NakLimitReached = 7,
    /// An inactivity timer expired.
    InactivityDetected = 8,
    /// Invalid file structure.
    InvalidFileStructure = 9,
    /// The check limit was reached.
    CheckLimitReached = 10,
    /// The requested checksum type is not supported.
    UnsupportedChecksumType = 11,
    /// A `SUSPEND` request was received for the transaction.
    SuspendReceived = 14,
    /// A `CANCEL` request was received for the transaction.
    CancelReceived = 15,
}

impl TryFrom<u8> for ConditionCode {
    type Error = CfdpError;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        let val = match val {
            0 => ConditionCode::NoError,
            1 => ConditionCode::AckLimitReached,
            2 => ConditionCode::KeepAliveLimitReached,
            3 => ConditionCode::InvalidTransmissionMode,
            4 => ConditionCode::FilestoreRejection,
            5 => ConditionCode::FileChecksumFailure,
            6 => ConditionCode::FileSizeError,
            7 => ConditionCode::NakLimitReached,
            8 => ConditionCode::InactivityDetected,
            9 => ConditionCode::InvalidFileStructure,
            10 => ConditionCode::CheckLimitReached,
            11 => ConditionCode::UnsupportedChecksumType,
            14 => ConditionCode::SuspendReceived,
            15 => ConditionCode::CancelReceived,
            _ => return Err(CfdpError::Custom("Invalid ConditionCode value")),
        };
        Ok(val)
    }
}
