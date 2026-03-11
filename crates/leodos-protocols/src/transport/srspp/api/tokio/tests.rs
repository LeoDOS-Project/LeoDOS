#![allow(unused)]

use crate::network::{NetworkRead, NetworkWrite};
use crate::network::isl::address::Address;
use crate::network::spp::Apid;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverMachine;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::rto::FixedRto;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;

use super::{SrsppReceiver, SrsppSender};

struct MockLinkPair;

impl MockLinkPair {
    fn new() -> (MockLinkA, MockLinkB) {
        let a_to_b = Arc::new(Mutex::new(VecDeque::new()));
        let b_to_a = Arc::new(Mutex::new(VecDeque::new()));

        let a = MockLinkA {
            send_queue: a_to_b.clone(),
            recv_queue: b_to_a.clone(),
        };
        let b = MockLinkB {
            send_queue: b_to_a,
            recv_queue: a_to_b,
        };

        (a, b)
    }
}

struct MockLinkA {
    send_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    recv_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

struct MockLinkB {
    send_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    recv_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

impl NetworkWrite for MockLinkA {
    type Error = std::io::Error;

    async fn write(&mut self, packet: &[u8]) -> Result<(), Self::Error> {
        self.send_queue.lock().await.push_back(packet.to_vec());
        Ok(())
    }
}

impl NetworkRead for MockLinkA {
    type Error = std::io::Error;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            if let Some(packet) = self.recv_queue.lock().await.pop_front() {
                let len = packet.len().min(buffer.len());
                buffer[..len].copy_from_slice(&packet[..len]);
                return Ok(len);
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
}

impl NetworkWrite for MockLinkB {
    type Error = std::io::Error;

    async fn write(&mut self, packet: &[u8]) -> Result<(), Self::Error> {
        self.send_queue.lock().await.push_back(packet.to_vec());
        Ok(())
    }
}

impl NetworkRead for MockLinkB {
    type Error = std::io::Error;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            if let Some(packet) = self.recv_queue.lock().await.pop_front() {
                let len = packet.len().min(buffer.len());
                buffer[..len].copy_from_slice(&packet[..len]);
                return Ok(len);
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
}

fn sender_config() -> SenderConfig {
    SenderConfig {
        source_address: Address::satellite(0, 1),
        apid: Apid::new(0x42).unwrap(),
        function_code: 0,
        rto_ticks: 1000,
        max_retransmits: 3,
        header_overhead: SrsppDataPacket::HEADER_SIZE,
    }
}

fn receiver_config() -> ReceiverConfig {
    ReceiverConfig {
        local_address: Address::satellite(0, 2),
        apid: Apid::new(0x42).unwrap(),
        function_code: 0,
        immediate_ack: true,
        ack_delay_ticks: 100,
        progress_timeout_ticks: None,
    }
}

fn remote_address() -> Address {
    Address::satellite(0, 1)
}

#[tokio::test]
async fn test_send_recv_single_message() {
    let (link_a, link_b) = MockLinkPair::new();

    let mut sender: SrsppSender<_, _, 8, 4096, 512> =
        SrsppSender::new(sender_config(), link_a, FixedRto::new(1000), 1000);
    let mut receiver: SrsppReceiver<_, ReceiverMachine<8, 4096, 8192>, 512> =
        SrsppReceiver::new(receiver_config(), remote_address(), link_b, 1000);

    let message = b"Hello, srspp!";
    let receiver_addr = Address::satellite(0, 2);

    let send_handle = tokio::spawn(async move {
        sender.send(receiver_addr, message).await.unwrap();
        sender.flush().await.unwrap();
        sender
    });

    let recv_handle = tokio::spawn(async move {
        let mut buf = [0u8; 8192];
        let len = receiver.recv(&mut buf).await.unwrap();
        let data = buf[..len].to_vec();
        (receiver, data)
    });

    let (sender, receiver) = tokio::join!(send_handle, recv_handle);
    let _sender = sender.unwrap();
    let (_receiver, received) = receiver.unwrap();
    assert_eq!(&received[..], message);
}
