use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use leodos_protocols::coding::{CodingRead, CodingWrite};
use leodos_protocols::datalink::framing::sdlp::tc::{
    BypassFlag, ControlFlag, TcFrameReader, TcFrameWriter,
    TcFrameWriterConfig,
};
use leodos_protocols::datalink::framing::sdlp::tm::{
    TmFrameReader, TmFrameWriter, TmFrameWriterConfig,
};
use leodos_protocols::datalink::link::framed::{
    DatalinkReader, DatalinkWriter,
};
use leodos_protocols::datalink::security::NoSecurity;
use leodos_protocols::datalink::{DatalinkRead, DatalinkWrite};
use leodos_protocols::ids::{Scid, Vcid};
use leodos_protocols::network::spp::{
    Apid, PacketType, SecondaryHeaderFlag, SequenceCount,
    SequenceFlag, SpacePacket,
};

fn build_space_packet(
    buf: &mut [u8],
    payload: &[u8],
) -> usize {
    let pkt = SpacePacket::builder()
        .buffer(buf)
        .apid(Apid::new(1).unwrap())
        .packet_type(PacketType::Telecommand)
        .sequence_count(SequenceCount::new())
        .secondary_header(SecondaryHeaderFlag::Absent)
        .sequence_flag(SequenceFlag::Unsegmented)
        .data_len(payload.len())
        .build()
        .unwrap();
    pkt.data_field_mut()[..payload.len()]
        .copy_from_slice(payload);
    pkt.primary_header.packet_len()
}

#[derive(Debug, Clone)]
struct MockError;

impl std::fmt::Display for MockError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "mock error")
    }
}

impl std::error::Error for MockError {}

struct MockChannelState {
    queue: VecDeque<Vec<u8>>,
}

struct MockChannel {
    state: Rc<RefCell<MockChannelState>>,
}

impl MockChannel {
    fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(MockChannelState {
                queue: VecDeque::new(),
            })),
        }
    }

    fn writer(&self) -> MockWriter {
        MockWriter {
            state: self.state.clone(),
        }
    }

    fn reader(&self) -> MockCodingRead {
        MockCodingRead {
            state: self.state.clone(),
        }
    }

    fn pop_front(&self) -> Option<Vec<u8>> {
        self.state.borrow_mut().queue.pop_front()
    }
}

struct MockWriter {
    state: Rc<RefCell<MockChannelState>>,
}

impl CodingWrite for MockWriter {
    type Error = MockError;

    async fn write(
        &mut self,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        self.state
            .borrow_mut()
            .queue
            .push_back(data.to_vec());
        Ok(())
    }
}

struct MockCodingRead {
    state: Rc<RefCell<MockChannelState>>,
}

impl CodingRead for MockCodingRead {
    type Error = MockError;

    async fn read(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<usize, Self::Error> {
        let frame = self.state.borrow_mut().queue.pop_front();
        match frame {
            Some(data) => {
                let len = data.len();
                buffer[..len].copy_from_slice(&data);
                Ok(len)
            }
            None => Ok(0),
        }
    }
}

#[test]
fn tc_sender_builds_valid_frame() {
    futures::executor::block_on(async {
        let config = TcFrameWriterConfig {
            scid: Scid::new(42),
            vcid: Vcid::new(1),
            bypass: BypassFlag::TypeA,
            control: ControlFlag::TypeD,
            max_data_field_len: 256,
        };

        let mock = MockChannel::new();
        let frame_writer = TcFrameWriter::<512>::new(config);
        let mut writer =
            DatalinkWriter::builder()
                .frame_writer(frame_writer)
                .coding_writer(mock.writer())
                .security(NoSecurity)
                .build();

        let mut pkt_buf = [0u8; 128];
        let pkt_len =
            build_space_packet(&mut pkt_buf, b"Hello, TC!");
        writer.write(&pkt_buf[..pkt_len]).await.unwrap();
        writer.flush().await.unwrap();

        let sent_frame = mock.pop_front().unwrap();
        assert!(sent_frame.len() > pkt_len);
    });
}

#[test]
fn tc_round_trip() {
    futures::executor::block_on(async {
        let config = TcFrameWriterConfig {
            scid: Scid::new(42),
            vcid: Vcid::new(1),
            bypass: BypassFlag::TypeA,
            control: ControlFlag::TypeD,
            max_data_field_len: 256,
        };

        let wire = MockChannel::new();
        let frame_writer = TcFrameWriter::<512>::new(config);
        let mut writer =
            DatalinkWriter::builder()
                .frame_writer(frame_writer)
                .coding_writer(wire.writer())
                .security(NoSecurity)
                .build();

        let mut pkt_buf = [0u8; 128];
        let payload = b"Hello, TC round trip!";
        let pkt_len =
            build_space_packet(&mut pkt_buf, payload);
        writer.write(&pkt_buf[..pkt_len]).await.unwrap();
        writer.flush().await.unwrap();

        let frame_reader = TcFrameReader::<512>::new();
        let mut reader =
            DatalinkReader::builder()
                .frame_reader(frame_reader)
                .coding_reader(wire.reader())
                .security(NoSecurity)
                .build();

        let mut recv_buf = [0u8; 512];
        let recv_len =
            reader.read(&mut recv_buf).await.unwrap();

        assert_eq!(
            &recv_buf[..recv_len],
            &pkt_buf[..pkt_len]
        );

        let parsed =
            SpacePacket::parse(&recv_buf[..recv_len]).unwrap();
        assert_eq!(
            &parsed.data_field()[..payload.len()],
            payload
        );
    });
}

#[test]
fn tm_sender_builds_valid_frame() {
    futures::executor::block_on(async {
        let config = TmFrameWriterConfig {
            scid: Scid::new(42),
            vcid: Vcid::new(1),
            max_data_field_len: 256,
        };

        let mock = MockChannel::new();
        let frame_writer = TmFrameWriter::<512>::new(config);
        let mut writer =
            DatalinkWriter::builder()
                .frame_writer(frame_writer)
                .coding_writer(mock.writer())
                .security(NoSecurity)
                .build();

        let mut pkt_buf = [0u8; 128];
        let pkt_len =
            build_space_packet(&mut pkt_buf, b"Hello, TM!");
        writer.write(&pkt_buf[..pkt_len]).await.unwrap();
        writer.flush().await.unwrap();

        let sent_frame = mock.pop_front().unwrap();
        assert!(sent_frame.len() > pkt_len);
    });
}

#[test]
fn tm_round_trip() {
    futures::executor::block_on(async {
        let config = TmFrameWriterConfig {
            scid: Scid::new(42),
            vcid: Vcid::new(1),
            max_data_field_len: 256,
        };

        let wire = MockChannel::new();
        let frame_writer = TmFrameWriter::<512>::new(config);
        let mut writer =
            DatalinkWriter::builder()
                .frame_writer(frame_writer)
                .coding_writer(wire.writer())
                .security(NoSecurity)
                .build();

        let mut pkt_buf = [0u8; 128];
        let payload = b"Hello, TM round trip!";
        let pkt_len =
            build_space_packet(&mut pkt_buf, payload);
        writer.write(&pkt_buf[..pkt_len]).await.unwrap();
        writer.flush().await.unwrap();

        let frame_reader = TmFrameReader::<512>::new();
        let mut reader =
            DatalinkReader::builder()
                .frame_reader(frame_reader)
                .coding_reader(wire.reader())
                .security(NoSecurity)
                .build();

        let mut recv_buf = [0u8; 512];
        let recv_len =
            reader.read(&mut recv_buf).await.unwrap();

        assert_eq!(
            &recv_buf[..recv_len],
            &pkt_buf[..pkt_len]
        );

        let parsed =
            SpacePacket::parse(&recv_buf[..recv_len]).unwrap();
        assert_eq!(
            &parsed.data_field()[..payload.len()],
            payload
        );
    });
}
