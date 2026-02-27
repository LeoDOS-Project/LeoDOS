use crate::transport::cfdp::CfdpError;

/// File Data PDU with segment metadata.
pub mod with_meta;
/// File Data PDU without segment metadata.
pub mod without_meta;

/// A parsed File Data PDU, dispatching between with-metadata and without-metadata variants.
#[derive(Debug, Clone, Copy)]
pub enum FileDataPdu<'a> {
    /// File Data PDU that includes segment metadata.
    WithMeta(&'a with_meta::FileDataPduWithMeta),
    /// File Data PDU without segment metadata.
    WithoutMeta(&'a without_meta::FileDataPduWithoutMeta),
}

impl<'a> FileDataPdu<'a> {
    /// Returns the file data payload bytes.
    pub fn file_data(&self, large_file_flag: bool) -> Result<&'a [u8], CfdpError> {
        match self {
            FileDataPdu::WithMeta(pdu) => pdu.file_data(large_file_flag),
            FileDataPdu::WithoutMeta(pdu) => pdu.file_data(large_file_flag),
        }
    }

    /// Returns the byte offset into the file for this data segment.
    pub fn offset(&self, large_file_flag: bool) -> Result<u64, CfdpError> {
        match self {
            FileDataPdu::WithMeta(pdu) => pdu.offset(large_file_flag),
            FileDataPdu::WithoutMeta(pdu) => pdu.offset(large_file_flag),
        }
    }
}
