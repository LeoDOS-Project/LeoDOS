//! Tokio-based async driver for srspp.
//!
//! Provides separate sender and receiver types for point-to-point communication.
//!
//! ## Sender Example
//!
//! ```ignore
//! let sender = SrsppSender::new(config, link);
//!
//! // Send messages
//! sender.send(&data).await?;
//! sender.send(&more_data).await?;
//!
//! // Wait for all to be acknowledged
//! sender.flush().await?;
//! ```
//!
//! ## Receiver Example
//!
//! ```ignore
//! let mut receiver = SrsppReceiver::new(config, link);
//!
//! // Receive messages
//! while let Some(message) = receiver.recv().await? {
//!     process(message);
//! }
//! ```

use crate::network::NetworkLayer;
use crate::network::isl::address::Address;
use crate::network::spp::Apid;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::receiver::ReceiverAction;
use crate::transport::srspp::machine::receiver::ReceiverActions;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverError;
use crate::transport::srspp::machine::receiver::ReceiverEvent;
use crate::transport::srspp::machine::receiver::ReceiverMachine;
use crate::transport::srspp::machine::sender::SenderAction;
use crate::transport::srspp::machine::sender::SenderActions;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::machine::sender::SenderError;
use crate::transport::srspp::machine::sender::SenderEvent;
use crate::transport::srspp::machine::sender::SenderMachine;
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::packet::parse_ack_packet;
use crate::transport::srspp::packet::parse_data_packet;
use crate::transport::srspp::packet::parse_srspp_type;
use std::collections::HashMap;
use tokio::time::Duration;
use tokio::time::Instant;

/// Error type for srspp operations.
#[derive(Debug)]
pub enum SrsppError {
    /// Send buffer is full.
    BufferFull,
    /// Window is full (too many unacked packets).
    WindowFull,
    /// Link error.
    LinkError(String),
    /// Packet error.
    PacketError(String),
    /// Sender error.
    SenderError(SenderError),
    /// Receiver error.
    ReceiverError(ReceiverError),
}

impl std::fmt::Display for SrsppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SrsppError::BufferFull => write!(f, "send buffer full"),
            SrsppError::WindowFull => write!(f, "window full"),
            SrsppError::LinkError(e) => write!(f, "link error: {}", e),
            SrsppError::PacketError(e) => write!(f, "packet error: {}", e),
            SrsppError::SenderError(e) => write!(f, "sender error: {:?}", e),
            SrsppError::ReceiverError(e) => write!(f, "receiver error: {:?}", e),
        }
    }
}

impl std::error::Error for SrsppError {}

impl From<SenderError> for SrsppError {
    fn from(e: SenderError) -> Self {
        SrsppError::SenderError(e)
    }
}

impl From<ReceiverError> for SrsppError {
    fn from(e: ReceiverError) -> Self {
        SrsppError::ReceiverError(e)
    }
}

/// Async srspp sender.
///
/// Sends messages reliably over the link, handling segmentation and retransmission.
/// Receives ACKs from the remote receiver.
pub struct SrsppSender<L: NetworkLayer, const WIN: usize, const BUF: usize, const MTU: usize> {
    link: L,
    machine: SenderMachine<WIN, BUF, MTU>,
    actions: SenderActions,
    retransmit_timers: HashMap<u16, Instant>,
    ticks_per_sec: u32,
    recv_buffer: [u8; MTU],
}

impl<L: NetworkLayer, const WIN: usize, const BUF: usize, const MTU: usize>
    SrsppSender<L, WIN, BUF, MTU>
{
    /// Create a new sender.
    pub fn new(config: SenderConfig, link: L, ticks_per_sec: u32) -> Self {
        Self {
            link,
            machine: SenderMachine::new(config),
            actions: SenderActions::new(),
            retransmit_timers: HashMap::new(),
            ticks_per_sec,
            recv_buffer: [0u8; MTU],
        }
    }

    /// Send a message.
    ///
    /// The message is segmented if necessary and queued for transmission.
    /// This returns when all packets have been transmitted (but not necessarily ACKed).
    ///
    /// For guaranteed delivery, call `flush()` after sending.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), SrsppError> {
        self.machine
            .handle(SenderEvent::SendRequest { data }, &mut self.actions)?;

        self.process_actions().await?;
        Ok(())
    }

    /// Wait for all sent data to be acknowledged.
    pub async fn flush(&mut self) -> Result<(), SrsppError> {
        while !self.machine.is_idle() {
            self.poll().await?;
        }
        Ok(())
    }

    /// Poll for incoming ACKs and handle timeouts.
    ///
    /// Call this periodically if you want to process ACKs without blocking on flush.
    pub async fn poll(&mut self) -> Result<(), SrsppError> {
        let next_deadline = self.next_timer_deadline();

        tokio::select! {
            biased;

            // Check for incoming ACK
            result = self.link.recv(&mut self.recv_buffer) => {
                let len = result.map_err(|e| SrsppError::LinkError(e.to_string()))?;
                self.handle_incoming(&self.recv_buffer[..len].to_vec()).await?;
            }

            // Handle timer expiration
            _ = sleep_until(next_deadline) => {
                self.handle_timeouts().await?;
            }
        }

        Ok(())
    }

    /// Check if all data has been acknowledged.
    pub fn is_idle(&self) -> bool {
        self.machine.is_idle()
    }

    /// Available buffer space in bytes.
    pub fn available_bytes(&self) -> usize {
        self.machine.available_bytes()
    }

    async fn process_actions(&mut self) -> Result<(), SrsppError> {
        for action in self.actions.iter() {
            match action {
                SenderAction::Transmit { seq, rto_ticks } => {
                    if let Some(packet) = self.machine.get_packet(*seq) {
                        self.link
                            .send(packet)
                            .await
                            .map_err(|e| SrsppError::LinkError(e.to_string()))?;

                        self.machine.mark_transmitted(*seq);

                        let deadline =
                            Instant::now() + ticks_to_duration(*rto_ticks, self.ticks_per_sec);
                        self.retransmit_timers.insert(seq.value(), deadline);
                    }
                }
                SenderAction::StopTimer { seq } => {
                    self.retransmit_timers.remove(&seq.value());
                }
                SenderAction::PacketLost { seq } => {
                    eprintln!("srspp: Packet {} lost after max retransmits", seq.value());
                }
                SenderAction::SpaceAvailable { .. } => {
                    // Could notify waiters if we add backpressure
                }
            }
        }
        Ok(())
    }

    async fn handle_incoming(&mut self, packet: &[u8]) -> Result<(), SrsppError> {
        let srspp_type =
            parse_srspp_type(packet).map_err(|e| SrsppError::PacketError(format!("{:?}", e)))?;

        if srspp_type == SrsppType::Ack {
            let ack =
                parse_ack_packet(packet).map_err(|e| SrsppError::PacketError(format!("{:?}", e)))?;

            self.machine.handle(
                SenderEvent::AckReceived {
                    cumulative_ack: ack.ack_payload.cumulative_ack(),
                    selective_bitmap: ack.ack_payload.selective_ack_bitmap(),
                },
                &mut self.actions,
            )?;

            self.process_actions().await?;
        }
        // Ignore non-ACK packets

        Ok(())
    }

    async fn handle_timeouts(&mut self) -> Result<(), SrsppError> {
        let now = Instant::now();

        let expired: Vec<u16> = self
            .retransmit_timers
            .iter()
            .filter(|(_, deadline)| **deadline <= now)
            .map(|(seq, _)| *seq)
            .collect();

        for seq_val in expired {
            self.retransmit_timers.remove(&seq_val);
            self.machine.handle(
                SenderEvent::RetransmitTimeout {
                    seq: SequenceCount::from(seq_val),
                },
                &mut self.actions,
            )?;
            self.process_actions().await?;
        }

        Ok(())
    }

    fn next_timer_deadline(&self) -> Option<Instant> {
        self.retransmit_timers.values().min().copied()
    }
}

/// Async srspp receiver.
///
/// Receives messages reliably over the link, handling reordering and reassembly.
/// Sends ACKs to the remote sender.
pub struct SrsppReceiver<
    L: NetworkLayer,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const REASM: usize,
> {
    link: L,
    local_address: Address,
    apid: Apid,
    machine: ReceiverMachine<WIN, BUF, REASM>,
    actions: ReceiverActions,
    ack_timer: Option<Instant>,
    ticks_per_sec: u32,
    recv_buffer: [u8; MTU],
    ack_buffer: [u8; 16],
}

impl<L: NetworkLayer, const WIN: usize, const BUF: usize, const MTU: usize, const REASM: usize>
    SrsppReceiver<L, WIN, BUF, MTU, REASM>
{
    /// Create a new receiver for a specific remote sender.
    pub fn new(
        config: ReceiverConfig,
        remote_address: Address,
        link: L,
        ticks_per_sec: u32,
    ) -> Self {
        let local_address = config.local_address;
        let apid = config.apid;
        Self {
            link,
            local_address,
            apid,
            machine: ReceiverMachine::new(config, remote_address),
            actions: ReceiverActions::new(),
            ack_timer: None,
            ticks_per_sec,
            recv_buffer: [0u8; MTU],
            ack_buffer: [0u8; 16],
        }
    }

    /// Receive the next complete message.
    ///
    /// Blocks until a message is available.
    pub async fn recv(&mut self) -> Result<Box<[u8]>, SrsppError> {
        loop {
            // Check if we already have a message
            if let Some(msg) = self.machine.take_message() {
                return Ok(msg.to_vec().into_boxed_slice());
            }

            // Wait for incoming packet or ACK timer
            self.poll().await?;
        }
    }

    /// Try to receive a message without blocking.
    ///
    /// Returns `None` if no complete message is available.
    pub fn try_recv(&mut self) -> Option<Box<[u8]>> {
        self.machine
            .take_message()
            .map(|m| m.to_vec().into_boxed_slice())
    }

    /// Poll for incoming data and handle ACK timer.
    pub async fn poll(&mut self) -> Result<(), SrsppError> {
        tokio::select! {
            biased;

            // Check for incoming data
            result = self.link.recv(&mut self.recv_buffer) => {
                let len = result.map_err(|e| SrsppError::LinkError(e.to_string()))?;
                self.handle_incoming(&self.recv_buffer[..len].to_vec()).await?;
            }

            // Handle ACK timer
            _ = sleep_until(self.ack_timer) => {
                self.handle_ack_timeout().await?;
            }
        }

        Ok(())
    }

    async fn handle_incoming(&mut self, packet: &[u8]) -> Result<(), SrsppError> {
        let srspp_type =
            parse_srspp_type(packet).map_err(|e| SrsppError::PacketError(format!("{:?}", e)))?;

        if srspp_type == SrsppType::Data {
            let data = parse_data_packet(packet)
                .map_err(|e| SrsppError::PacketError(format!("{:?}", e)))?;

            self.machine.handle(
                ReceiverEvent::DataReceived {
                    seq: data.primary.sequence_count(),
                    flags: data.primary.sequence_flag(),
                    payload: &data.payload,
                },
                &mut self.actions,
            )?;

            self.process_actions().await?;
        }
        // Ignore non-DATA packets

        Ok(())
    }

    async fn handle_ack_timeout(&mut self) -> Result<(), SrsppError> {
        self.ack_timer = None;
        self.machine
            .handle(ReceiverEvent::AckTimeout, &mut self.actions)?;
        self.process_actions().await?;
        Ok(())
    }

    async fn process_actions(&mut self) -> Result<(), SrsppError> {
        for action in self.actions.iter() {
            match action {
                ReceiverAction::SendAck {
                    destination: _,
                    cumulative_ack,
                    selective_bitmap,
                } => {
                    let ack = SrsppAckPacket::builder()
                        .buffer(&mut self.ack_buffer)
                        .source_address(self.local_address)
                        .apid(self.apid)
                        .cumulative_ack(*cumulative_ack)
                        .sequence_count(SequenceCount::new())
                        .selective_bitmap(*selective_bitmap)
                        .build()
                        .map_err(|e| SrsppError::PacketError(format!("{:?}", e)))?;

                    self.link
                        .send(zerocopy::IntoBytes::as_bytes(ack))
                        .await
                        .map_err(|e| SrsppError::LinkError(e.to_string()))?;
                }
                ReceiverAction::StartAckTimer { ticks } => {
                    self.ack_timer =
                        Some(Instant::now() + ticks_to_duration(*ticks, self.ticks_per_sec));
                }
                ReceiverAction::StopAckTimer => {
                    self.ack_timer = None;
                }
                ReceiverAction::MessageReady => {
                    // Message will be retrieved by recv()
                }
            }
        }
        Ok(())
    }
}

fn ticks_to_duration(ticks: u32, ticks_per_sec: u32) -> Duration {
    Duration::from_millis((ticks as u64 * 1000) / ticks_per_sec as u64)
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(d) => tokio::time::sleep_until(d.into()).await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused)]
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// A pair of linked mock links for testing.
    struct MockLinkPair {
        a_to_b: Arc<Mutex<VecDeque<Vec<u8>>>>,
        b_to_a: Arc<Mutex<VecDeque<Vec<u8>>>>,
    }

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

    impl NetworkLayer for MockLinkA {
        type Error = std::io::Error;

        async fn send(&mut self, packet: &[u8]) -> Result<(), Self::Error> {
            self.send_queue.lock().await.push_back(packet.to_vec());
            Ok(())
        }

        async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
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

    impl NetworkLayer for MockLinkB {
        type Error = std::io::Error;

        async fn send(&mut self, packet: &[u8]) -> Result<(), Self::Error> {
            self.send_queue.lock().await.push_back(packet.to_vec());
            Ok(())
        }

        async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
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
            rto_ticks: 1000,
            max_retransmits: 3,
        }
    }

    fn receiver_config() -> ReceiverConfig {
        ReceiverConfig {
            local_address: Address::satellite(0, 2),
            apid: Apid::new(0x42).unwrap(),
            immediate_ack: true,
            ack_delay_ticks: 100,
        }
    }

    fn remote_address() -> Address {
        Address::satellite(0, 1)
    }

    #[tokio::test]
    async fn test_send_recv_single_message() {
        let (link_a, link_b) = MockLinkPair::new();

        let mut sender: SrsppSender<_, 8, 4096, 512> =
            SrsppSender::new(sender_config(), link_a, 1000);
        let mut receiver: SrsppReceiver<_, 8, 4096, 512, 8192> =
            SrsppReceiver::new(receiver_config(), remote_address(), link_b, 1000);

        let message = b"Hello, srspp!";

        // Send in one task
        let send_handle = tokio::spawn(async move {
            sender.send(message).await.unwrap();
            sender.flush().await.unwrap();
            sender
        });

        // Receive in another
        let recv_handle = tokio::spawn(async move {
            let received = receiver.recv().await.unwrap();
            (receiver, received)
        });

        let (sender, receiver) = tokio::join!(send_handle, recv_handle);
        let _sender = sender.unwrap();
        let (_receiver, received) = receiver.unwrap();
        assert_eq!(&*received, message);
    }
}
