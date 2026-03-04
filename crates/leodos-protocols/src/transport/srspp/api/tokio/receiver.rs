use crate::network::NetworkLayer;
use crate::network::isl::address::Address;
use crate::network::spp::Apid;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::receiver::ReceiverAction;
use crate::transport::srspp::machine::receiver::ReceiverActions;
use crate::transport::srspp::machine::receiver::ReceiverBackend;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverEvent;
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::packet::parse_data_packet;
use crate::transport::srspp::packet::parse_srspp_type;
use tokio::time::Instant;

use super::SrsppError;
use super::sleep_until;
use super::ticks_to_duration;

/// Async srspp receiver.
///
/// Receives messages reliably over the link, handling reordering and reassembly.
/// Sends ACKs to the remote sender.
pub struct SrsppReceiver<L: NetworkLayer, R: ReceiverBackend, const MTU: usize> {
    link: L,
    local_address: Address,
    apid: Apid,
    function_code: u8,
    message_id: u8,
    action_code: u8,
    machine: R,
    actions: ReceiverActions,
    ack_timer: Option<Instant>,
    progress_timer: Option<Instant>,
    ticks_per_sec: u32,
    recv_buffer: [u8; MTU],
    ack_buffer: [u8; 32],
}

impl<L: NetworkLayer, R: ReceiverBackend, const MTU: usize> SrsppReceiver<L, R, MTU> {
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
        let message_id = config.message_id;
        let action_code = config.action_code;
        Self {
            link,
            local_address,
            apid,
            function_code,
            message_id,
            action_code,
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

    /// Poll for incoming data and handle timers.
    pub async fn poll(&mut self) -> Result<(), SrsppError> {
        tokio::select! {
            biased;

            result = self.link.recv(&mut self.recv_buffer) => {
                let len = result.map_err(|e| SrsppError::LinkError(e.to_string()))?;
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

        Ok(())
    }

    async fn handle_ack_timeout(&mut self) -> Result<(), SrsppError> {
        self.ack_timer = None;
        self.machine
            .handle(ReceiverEvent::AckTimeout, &mut self.actions)?;
        self.process_actions().await?;
        Ok(())
    }

    async fn handle_progress_timeout(&mut self) -> Result<(), SrsppError> {
        self.progress_timer = None;
        self.machine
            .handle(ReceiverEvent::ProgressTimeout, &mut self.actions)?;
        self.process_actions().await?;
        Ok(())
    }

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
                        .message_id(self.message_id)
                        .action_code(self.action_code)
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
