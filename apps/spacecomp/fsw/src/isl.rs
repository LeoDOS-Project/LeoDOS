use core::mem::size_of;

use leodos_protocols::mission::compute::packet::{OpCode, SpaceCompHeader};
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::routing::packet::IslRoutingTelecommand;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::network::NetworkLayer;
use zerocopy::network_endian::U16;
use zerocopy::FromBytes;
use zerocopy::IntoBytes;

pub struct Context {
    pub local_address: Address,
    pub apid: Apid,
}

pub async fn send<L: NetworkLayer>(
    link: &mut L,
    ctx: &Context,
    target: Address,
    op_code: OpCode,
    job_id: u16,
    payload: &[u8],
) -> Result<(), L::Error> {
    let mut buf = [0u8; 512];
    let inner_len = size_of::<SpaceCompHeader>() + payload.len();

    let isl = match IslRoutingTelecommand::builder()
        .buffer(&mut buf)
        .apid(ctx.apid)
        .function_code(0)
        .message_id(0)
        .target(target)
        .action_code(0)
        .payload_len(inner_len)
        .build()
    {
        Ok(isl) => isl,
        Err(_) => return Ok(()),
    };

    let isl_payload = isl.payload_mut();
    let header = SpaceCompHeader {
        op_code: op_code as u8,
        _reserved: 0,
        job_id: U16::new(job_id),
    };
    isl_payload[..size_of::<SpaceCompHeader>()].copy_from_slice(header.as_bytes());
    if !payload.is_empty() {
        let start = size_of::<SpaceCompHeader>();
        isl_payload[start..start + payload.len()].copy_from_slice(payload);
    }
    isl.set_cfe_checksum();

    link.send(isl.as_bytes()).await
}

pub struct Parsed {
    pub op_code: OpCode,
    pub job_id: u16,
    pub payload_len: usize,
}

pub fn parse_and_copy(raw: &[u8], payload_out: &mut [u8]) -> Option<Parsed> {
    let isl_tc = IslRoutingTelecommand::parse(raw).ok()?;
    let inner = isl_tc.payload();
    let hdr_size = size_of::<SpaceCompHeader>();
    if inner.len() < hdr_size {
        return None;
    }
    let header = SpaceCompHeader::read_from_bytes(&inner[..hdr_size]).ok()?;
    let op_code = header.op_code().ok()?;
    let job_id = header.job_id();
    let payload = &inner[hdr_size..];
    let plen = payload.len().min(payload_out.len());
    payload_out[..plen].copy_from_slice(&payload[..plen]);
    Some(Parsed {
        op_code,
        job_id,
        payload_len: plen,
    })
}
