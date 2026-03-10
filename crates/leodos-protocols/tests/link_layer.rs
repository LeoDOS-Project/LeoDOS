use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use leodos_protocols::coding::CodingWriter;
use leodos_protocols::datalink::link::tc::{TcConfig, TcWriteChannel};
use leodos_protocols::datalink::link::tm::{TmConfig, TmWriteChannel};
use leodos_protocols::datalink::sdlp::tc::{BypassFlag, ControlFlag};

#[derive(Debug, Clone)]
struct MockError;

impl std::fmt::Display for MockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

    fn pop_front(&self) -> Option<Vec<u8>> {
        self.state.borrow_mut().queue.pop_front()
    }
}

struct MockWriter {
    state: Rc<RefCell<MockChannelState>>,
}

impl CodingWriter for MockWriter {
    type Error = MockError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.state.borrow_mut().queue.push_back(data.to_vec());
        Ok(())
    }
}

#[test]
fn tc_sender_builds_valid_frame() {
    futures::executor::block_on(async {
        let config = TcConfig {
            scid: 42,
            vcid: 1,
            bypass: BypassFlag::TypeA,
            control: ControlFlag::TypeD,
            max_frame_data_len: 256,
        };

        let mock = MockChannel::new();
        let writer = mock.writer();

        let channel: TcWriteChannel<MockError, 8, 512> =
            TcWriteChannel::new(config);
        let (mut handle, mut driver) = channel.split(writer);

        let test_data = b"Hello, TC!";
        handle.send(test_data).await.unwrap();
        handle.close();

        driver.run().await.unwrap();

        let sent_frame = mock.pop_front().unwrap();
        assert!(sent_frame.len() > test_data.len());
        assert!(sent_frame[..]
            .windows(test_data.len())
            .any(|w| w == test_data));
    });
}

#[test]
fn tc_round_trip() {
    futures::executor::block_on(async {
        let config = TcConfig {
            scid: 42,
            vcid: 1,
            bypass: BypassFlag::TypeA,
            control: ControlFlag::TypeD,
            max_frame_data_len: 256,
        };

        let wire = MockChannel::new();

        let send_channel: TcWriteChannel<MockError, 8, 512> =
            TcWriteChannel::new(config.clone());
        let (mut send_handle, mut send_driver) =
            send_channel.split(wire.writer());

        let test_data = b"Hello, TC round trip!";
        send_handle.send(test_data).await.unwrap();
        send_handle.close();
        send_driver.run().await.unwrap();

        let sent_frame = wire.pop_front().unwrap();

        let frame =
            leodos_protocols::datalink::sdlp::tc::TelecommandTransferFrame::parse(&sent_frame)
                .unwrap();
        let data_field = frame.data_field();

        assert_eq!(data_field, test_data);
    });
}

#[test]
fn tm_sender_builds_valid_frame() {
    futures::executor::block_on(async {
        let config = TmConfig {
            scid: 42,
            vcid: 1,
            max_frame_data_len: 256,
        };

        let mock = MockChannel::new();
        let writer = mock.writer();

        let channel: TmWriteChannel<MockError, 8, 512> =
            TmWriteChannel::new(config);
        let (mut handle, mut driver) = channel.split(writer);

        let test_data = b"Hello, TM!";
        handle.send(test_data).await.unwrap();
        handle.close();

        driver.run().await.unwrap();

        let sent_frame = mock.pop_front().unwrap();
        assert!(sent_frame.len() > test_data.len());
    });
}

#[test]
fn tm_round_trip() {
    futures::executor::block_on(async {
        let config = TmConfig {
            scid: 42,
            vcid: 1,
            max_frame_data_len: 256,
        };

        let wire = MockChannel::new();

        let send_channel: TmWriteChannel<MockError, 8, 512> =
            TmWriteChannel::new(config.clone());
        let (mut send_handle, mut send_driver) =
            send_channel.split(wire.writer());

        let test_data = b"Hello, TM round trip!";
        send_handle.send(test_data).await.unwrap();
        send_handle.close();
        send_driver.run().await.unwrap();

        let sent_frame = wire.pop_front().unwrap();

        // Since no coding pipeline was used (MockWriter passes
        // through), the frame is not randomized.
        let frame =
            leodos_protocols::datalink::sdlp::tm::TelemetryTransferFrame::parse_raw(&sent_frame)
                .unwrap();

        assert_eq!(frame.data_field(), test_data);
    });
}
