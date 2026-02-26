use core::mem::size_of;

use leodos_protocols::mission::compute::packet::{OpCode, SpaceCompHeader};
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::routing::local::LocalLinkError;
use leodos_protocols::transport::srspp::api::cfs::{Error, SrsppNodeHandle};
use zerocopy::network_endian::U16;
use zerocopy::FromBytes;
use zerocopy::IntoBytes;

pub type NodeHandle<'a> = SrsppNodeHandle<'a, LocalLinkError, 8, 4096, 512, 8192, 4>;

const MAX_ISL_MESSAGE: usize = 512;

pub async fn send<
    E: Clone,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const REASM: usize,
    const MAX_STREAMS: usize,
>(
    handle: &mut SrsppNodeHandle<'_, E, WIN, BUF, MTU, REASM, MAX_STREAMS>,
    target: Address,
    op_code: OpCode,
    job_id: u16,
    payload: &[u8],
) -> Result<(), Error<E>> {
    let mut buf = [0u8; MAX_ISL_MESSAGE];
    let hdr_size = size_of::<SpaceCompHeader>();
    let header = SpaceCompHeader {
        op_code: op_code as u8,
        _reserved: 0,
        job_id: U16::new(job_id),
    };
    buf[..hdr_size].copy_from_slice(header.as_bytes());
    if !payload.is_empty() {
        buf[hdr_size..hdr_size + payload.len()].copy_from_slice(payload);
    }
    let total = hdr_size + payload.len();
    handle.send(target, &buf[..total]).await
}

pub struct Parsed {
    pub op_code: OpCode,
    pub job_id: u16,
    pub payload_len: usize,
}

pub fn parse(data: &[u8], payload_out: &mut [u8]) -> Option<Parsed> {
    let hdr_size = size_of::<SpaceCompHeader>();
    if data.len() < hdr_size {
        return None;
    }
    let header = SpaceCompHeader::read_from_bytes(&data[..hdr_size]).ok()?;
    let op_code = header.op_code().ok()?;
    let job_id = header.job_id();
    let payload = &data[hdr_size..];
    let plen = payload.len().min(payload_out.len());
    payload_out[..plen].copy_from_slice(&payload[..plen]);
    Some(Parsed {
        op_code,
        job_id,
        payload_len: plen,
    })
}
