use crate::transport::cfdp::CfdpError;

pub mod with_meta;
pub mod without_meta;

#[derive(Debug, Clone, Copy)]
pub enum FileDataPdu<'a> {
    WithMeta(&'a with_meta::FileDataPduWithMeta),
    WithoutMeta(&'a without_meta::FileDataPduWithoutMeta),
}

impl<'a> FileDataPdu<'a> {
    pub fn file_data(&self, large_file_flag: bool) -> Result<&'a [u8], CfdpError> {
        match self {
            FileDataPdu::WithMeta(pdu) => pdu.file_data(large_file_flag),
            FileDataPdu::WithoutMeta(pdu) => pdu.file_data(large_file_flag),
        }
    }

    pub fn offset(&self, large_file_flag: bool) -> Result<u64, CfdpError> {
        match self {
            FileDataPdu::WithMeta(pdu) => pdu.offset(large_file_flag),
            FileDataPdu::WithoutMeta(pdu) => pdu.offset(large_file_flag),
        }
    }
}
