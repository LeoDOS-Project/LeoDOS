use crate::network::{NetworkRead, NetworkWrite};
use crate::network::isl::address::Address;
use crate::network::spp::Apid;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::receiver::ReceiverAction;
use crate::transport::srspp::machine::receiver::ReceiverActions;
use crate::transport::srspp::machine::receiver::ReceiverBackend;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverEvent;
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::packet::SrsppPacket;
use crate::transport::srspp::packet::SrsppType;
use tokio::time::Instant;

use super::SrsppError;
use super::sleep_until;
use super::ticks_to_duration;

/// Async srspp receiver.
///
/// Receives messages reliably over the link, handling reordering and reassembly.
/// Sends ACKs to the remote sender.
pub struct SrsppReceiver<L: NetworkWrite + NetworkRead<Error = <L as NetworkWrite>::Error>, R: ReceiverBackend, const MTU: usize> {
    /// Network link for receiving data and sending ACKs.
    link: L,
    /// Local address used as the source in outgoing ACKs.
    local_address: Address,
    /// APID used in outgoing ACK packets.
    apid: Apid,
    /// Function code used in outgoing ACK packets.
    function_code: u8,
    /// Receiver state machine handling reordering and reassembly.
    machine: R,
    /// Pending actions from the state machine.
    actions: ReceiverActions,
    /// Deadline for the delayed ACK timer.
    ack_timer: Option<Instant>,
    /// Deadline for the progress (inactivity) timer.
    progress_timer: Option<Instant>,
    /// Tick rate used to convert timer ticks to durations.
    ticks_per_sec: u32,
    /// Buffer for receiving data packets from the link.
    recv_buffer: [u8; MTU],
    /// Buffer for building outgoing ACK packets.
    ack_buffer: [u8; 32],
}

impl<L: NetworkWrite + NetworkRead<Error = <L as NetworkWrite>::Error>, R: ReceiverBackend, const MTU: usize> SrsppReceiver<L, R, MTU> {
    /// Create a new receiver for a specific remote sender.
    pub fn new(
        config: ReceiverConfig,
        remote_address: Address,
        link: L,
        ticks_per_sec: u32,
    ) -> Self {
        let local_address = config.local_address;
        let apid = config.apid;
        let function_code = config.function_code;
        Self {
            link,
            local_address,
            apid,
            function_code,
            machine: R::new(config, remote_address),
            actions: ReceiverActions::new(),
            ack_timer: None,
            progress_timer: None,
            ticks_per_sec,
            recv_buffer: [0u8; MTU],
            ack_buffer: [0u8; 32],
        }
    }

    /// Receive the next complete message.
    ///
    /// Blocks until a message is available.
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, SrsppError> {
        loop {
            if let Some(msg) = self.machine.take_message() {
                let len = msg.len().min(buf.len());
                buf[..len].copy_from_slice(&msg[..len]);
                return Ok(len);
            }

            self.poll().await?;
        }
    }

    /// Try to receive a message without blocking.
    ///
    /// Returns `None` if no complete message is available.
    pub fn try_recv(&mut self, buf: &mut [u8]) -> Option<usize> {
        self.machine.take_message().map(|m| {
            let len = m.len().min(buf.len());
            buf[..len].copy_from_slice(&m[..len]);
            len
        })
    }

    /// Wait for a complete message to become available.
    ///
    /// Returns a [`DeliveryToken`] that borrows `&mut self`,
    /// preventing further receives while the token is held.
    /// Call [`DeliveryToken::consume`] with a synchronous closure
    /// to read the message data without an intermediate copy.
    pub async fn wait_for_message(
        &mut self,
    ) -> Result<DeliveryToken<'_, L, R, MTU>, SrsppError> {
        loop {
            if let Some(len) = self.machine.message_len() {
                return Ok(DeliveryToken {
                    rx: self,
                    msg_len: len,
                });
            }
            self.poll().await?;
        }
    }

    /// Wait for a message and process it in-place with a closure.
    ///
    /// Equivalent to `wait_for_message().await?.consume(f)` but
    /// more concise when you don't need the [`DeliveryToken`]
    /// metadata.
    pub async fn recv_with<F, Ret>(&mut self, f: F) -> Result<Ret, SrsppError>
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        let token = self.wait_for_message().await?;
        Ok(token.consume(f))
    }

    /// Poll for incoming data and handle timers.
    pub async fn poll(&mut self) -> Result<(), SrsppError> {
        tokio::select! {
            biased;

            result = self.link.read(&mut self.recv_buffer) => {
                let len = result.map_err(|e| SrsppError::Network(e.to_string()))?;
                self.handle_incoming(&self.recv_buffer[..len].to_vec()).await?;
            }

            _ = sleep_until(self.ack_timer) => {
                self.handle_ack_timeout().await?;
            }

            _ = sleep_until(self.progress_timer) => {
                self.handle_progress_timeout().await?;
            }
        }

        Ok(())
    }

    /// Parses an incoming packet and processes it if it is a data packet.
    async fn handle_incoming(&mut self, packet: &[u8]) -> Result<(), SrsppError> {
        let parsed = SrsppPacket::parse(packet)
            .map_err(|e| SrsppError::PacketError(format!("{:?}", e)))?;
        let srspp_type = parsed.srspp_type()
            .map_err(|e| SrsppError::PacketError(format!("{:?}", e)))?;

        if srspp_type == SrsppType::Data {
            let data = SrsppDataPacket::parse(packet)
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

        Ok(())
    }

    /// Fires when the delayed ACK timer expires and sends an ACK.
    async fn handle_ack_timeout(&mut self) -> Result<(), SrsppError> {
        self.ack_timer = None;
        self.machine
            .handle(ReceiverEvent::AckTimeout, &mut self.actions)?;
        self.process_actions().await?;
        Ok(())
    }

    /// Fires when the progress timer expires due to sender inactivity.
    async fn handle_progress_timeout(&mut self) -> Result<(), SrsppError> {
        self.progress_timer = None;
        self.machine
            .handle(ReceiverEvent::ProgressTimeout, &mut self.actions)?;
        self.process_actions().await?;
        Ok(())
    }

    /// Executes pending actions: sends ACKs and manages timers.
    async fn process_actions(&mut self) -> Result<(), SrsppError> {
        for action in self.actions.iter() {
            match action {
                ReceiverAction::SendAck {
                    destination,
                    cumulative_ack,
                    selective_bitmap,
                } => {
                    let ack = SrsppAckPacket::builder()
                        .buffer(&mut self.ack_buffer)
                        .source_address(self.local_address)
                        .target(*destination)
                        .apid(self.apid)
                        .function_code(self.function_code)
                        .cumulative_ack(*cumulative_ack)
                        .sequence_count(SequenceCount::new())
                        .selective_bitmap(*selective_bitmap)
                        .build()
                        .map_err(|e| SrsppError::PacketError(format!("{:?}", e)))?;

                    self.link
                        .write(zerocopy::IntoBytes::as_bytes(ack))
                        .await
                        .map_err(|e| SrsppError::Network(e.to_string()))?;
                }
                ReceiverAction::StartAckTimer { ticks } => {
                    self.ack_timer =
                        Some(Instant::now() + ticks_to_duration(*ticks, self.ticks_per_sec));
                }
                ReceiverAction::StopAckTimer => {
                    self.ack_timer = None;
                }
                ReceiverAction::MessageReady => {
                }
                ReceiverAction::StartProgressTimer { ticks } => {
                    self.progress_timer =
                        Some(Instant::now() + ticks_to_duration(*ticks, self.ticks_per_sec));
                }
                ReceiverAction::StopProgressTimer => {
                    self.progress_timer = None;
                }
            }
        }
        Ok(())
    }
}

/// Zero-copy delivery token returned by
/// [`SrsppReceiver::wait_for_message`].
///
/// Holds `&mut SrsppReceiver`, preventing any I/O while the
/// token is alive.  Call [`consume`](Self::consume) with a
/// synchronous closure to read the message and release the
/// token in one step.
pub struct DeliveryToken<'a, L: NetworkWrite + NetworkRead<Error = <L as NetworkWrite>::Error>, R: ReceiverBackend, const MTU: usize> {
    rx: &'a mut SrsppReceiver<L, R, MTU>,
    msg_len: usize,
}

impl<'a, L: NetworkWrite + NetworkRead<Error = <L as NetworkWrite>::Error>, R: ReceiverBackend, const MTU: usize>
    DeliveryToken<'a, L, R, MTU>
{
    /// Byte length of the pending message.
    pub fn len(&self) -> usize {
        self.msg_len
    }

    /// Pass the message data to `f`, consume the token, and
    /// return whatever `f` returns.
    pub fn consume<F, Ret>(self, f: F) -> Ret
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        self.rx.machine.consume_message(f).unwrap()
    }
}
