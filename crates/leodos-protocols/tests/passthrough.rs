use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::poll_fn;
use std::rc::Rc;
use std::task::Poll;

use leodos_protocols::datalink::link::asymmetric::AsymmetricLink;
use leodos_protocols::datalink::link::{FrameReceiver, FrameSender};
use leodos_protocols::datalink::{DataLinkReader, DataLinkWriter};
use leodos_protocols::network::passthrough::PassThrough;
use leodos_protocols::network::{NetworkReader, NetworkWriter};

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

    fn split(&self) -> (MockSender, MockReceiver) {
        (
            MockSender {
                state: self.state.clone(),
            },
            MockReceiver {
                state: self.state.clone(),
            },
        )
    }
}

struct MockSender {
    state: Rc<RefCell<MockChannelState>>,
}

struct MockReceiver {
    state: Rc<RefCell<MockChannelState>>,
}

impl FrameSender for MockSender {
    type Error = MockError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.state.borrow_mut().queue.push_back(data.to_vec());
        Ok(())
    }
}

impl FrameReceiver for MockReceiver {
    type Error = MockError;

    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        poll_fn(|_cx| {
            let mut state = self.state.borrow_mut();
            if let Some(data) = state.queue.pop_front() {
                let len = data.len().min(buffer.len());
                buffer[..len].copy_from_slice(&data[..len]);
                return Poll::Ready(Ok(len));
            }
            Poll::Pending
        })
        .await
    }
}

#[test]
fn asymmetric_link_send() {
    futures::executor::block_on(async {
        let send_channel = MockChannel::new();
        let recv_channel = MockChannel::new();

        let (sender, _) = send_channel.split();
        let (_, receiver) = recv_channel.split();

        let mut link = AsymmetricLink::new(sender, receiver);

        let test_data = b"Test message";
        link.send(test_data).await.unwrap();

        let sent = send_channel.state.borrow_mut().queue.pop_front().unwrap();
        assert_eq!(&sent[..], test_data);
    });
}

#[test]
fn asymmetric_link_recv() {
    futures::executor::block_on(async {
        let send_channel = MockChannel::new();
        let recv_channel = MockChannel::new();

        let (sender, _) = send_channel.split();
        let (_, receiver) = recv_channel.split();

        let mut link = AsymmetricLink::new(sender, receiver);

        let test_data = b"Test message";
        recv_channel
            .state
            .borrow_mut()
            .queue
            .push_back(test_data.to_vec());

        let mut buffer = [0u8; 256];
        let len = link.recv(&mut buffer).await.unwrap();

        assert_eq!(&buffer[..len], test_data);
    });
}

#[test]
fn passthrough_send() {
    futures::executor::block_on(async {
        let send_channel = MockChannel::new();
        let recv_channel = MockChannel::new();

        let (sender, _) = send_channel.split();
        let (_, receiver) = recv_channel.split();

        let link = AsymmetricLink::new(sender, receiver);
        let mut passthrough = PassThrough::new(link);

        let test_data = b"PassThrough test";
        passthrough.send(test_data).await.unwrap();

        let sent = send_channel.state.borrow_mut().queue.pop_front().unwrap();
        assert_eq!(&sent[..], test_data);
    });
}

#[test]
fn passthrough_recv() {
    futures::executor::block_on(async {
        let send_channel = MockChannel::new();
        let recv_channel = MockChannel::new();

        let (sender, _) = send_channel.split();
        let (_, receiver) = recv_channel.split();

        let link = AsymmetricLink::new(sender, receiver);
        let mut passthrough = PassThrough::new(link);

        let test_data = b"PassThrough recv test";
        recv_channel
            .state
            .borrow_mut()
            .queue
            .push_back(test_data.to_vec());

        let mut buffer = [0u8; 256];
        let len = passthrough.recv(&mut buffer).await.unwrap();

        assert_eq!(&buffer[..len], test_data);
    });
}

#[test]
fn passthrough_into_inner() {
    let send_channel = MockChannel::new();
    let recv_channel = MockChannel::new();

    let (sender, _) = send_channel.split();
    let (_, receiver) = recv_channel.split();

    let link = AsymmetricLink::new(sender, receiver);
    let passthrough = PassThrough::new(link);

    let _recovered_link = passthrough.into_inner();
}
