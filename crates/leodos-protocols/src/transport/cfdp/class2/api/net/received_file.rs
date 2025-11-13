use crate::cfdp::pdu::EntityId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedFile {
    pub file_name: heapless::String<256>,
    pub length: u64,
    pub remote_entity_id: EntityId,
}
