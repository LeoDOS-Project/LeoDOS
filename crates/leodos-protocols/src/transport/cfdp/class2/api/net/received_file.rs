use crate::cfdp::pdu::EntityId;

/// Metadata about a successfully received file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedFile {
    /// The destination file name in the local filestore.
    pub file_name: heapless::String<256>,
    /// The total size of the received file in bytes.
    pub length: u64,
    /// The CFDP entity ID of the sender.
    pub remote_entity_id: EntityId,
}
