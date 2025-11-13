use zerocopy::FromBytes;
use zerocopy::IntoBytes as _;

use crate::datalink::DataLink;
use crate::network::spp;
use crate::network::spp::Apid;
use crate::network::spp::PacketType;
use crate::network::spp::SecondaryHeaderFlag;
use crate::network::spp::SequenceCount;
use crate::network::spp::SequenceFlag;
use crate::network::spp::SpacePacket;

pub struct SpacePacketSender<L2> {
    datalink: L2,
    apid: Apid,
    packet_type: PacketType,
    seq_count: SequenceCount,
    buffer: [u8; 2048],
}

pub enum Error<L2: DataLink> {
    DatalinkError(L2::Error),
    SpacePacketError(spp::BuildError),
}

impl<L2: DataLink> SpacePacketSender<L2> {
    pub fn new(datalink: L2, apid: u16, packet_type: PacketType) -> Self {
        Self {
            datalink,
            apid: Apid::new(apid).unwrap(),
            packet_type,
            seq_count: SequenceCount::new(),
            buffer: [0u8; 2048],
        }
    }
}

impl<L2: DataLink> SpacePacketSender<L2> {
    pub async fn send(
        &mut self,
        payload_len: usize,
        payload: impl Fn(&mut [u8]),
    ) -> Result<(), Error<L2>> {
        let packet = SpacePacket::builder()
            .buffer(&mut self.buffer)
            .apid(self.apid)
            .sequence_count(self.seq_count)
            .data_len(payload_len)
            .packet_type(self.packet_type)
            .secondary_header(SecondaryHeaderFlag::Present)
            .sequence_flag(SequenceFlag::Unsegmented)
            .build()
            .map_err(Error::SpacePacketError)?;

        payload(packet.data_field_mut());
        self.seq_count.increment();
        self.datalink
            .send(packet.as_bytes())
            .await
            .map_err(Error::DatalinkError)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum ReceiveError<L2Err> {
    Datalink(L2Err),
    ChecksumInvalid,
    ApidMismatch,
    FormatError,
}

pub struct SpacePacketReceiver<L2> {
    datalink: L2,
    my_apid: Apid,
}

impl<L2: DataLink> SpacePacketReceiver<L2> {
    pub fn new(datalink: L2, apid: u16) -> Self {
        Self {
            datalink,
            my_apid: Apid::new(apid).unwrap(),
        }
    }
}

impl<L2: DataLink> SpacePacketReceiver<L2> {
    pub async fn recv<'a>(
        &mut self,
        buffer: &'a mut [u8],
    ) -> Result<&'a SpacePacket, ReceiveError<L2::Error>> {
        loop {
            let len = self
                .datalink
                .recv(buffer)
                .await
                .map_err(ReceiveError::Datalink)?;

            let Ok((sp, _)) = SpacePacket::ref_from_prefix_with_elems(buffer, len) else {
                return Err(ReceiveError::FormatError);
            };

            if sp.apid() != self.my_apid {
                return Err(ReceiveError::ApidMismatch);
            }

            return Ok(sp);
        }
    }
}
