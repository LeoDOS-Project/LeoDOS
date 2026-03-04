use zerocopy::{Immutable, IntoBytes};

use crate::network::NetworkLayer;
use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::sender::SenderAction;
use crate::transport::srspp::machine::sender::SenderActions;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::machine::sender::SenderEvent;
use crate::transport::srspp::machine::sender::SenderMachine;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::packet::parse_ack_packet;
use crate::transport::srspp::packet::parse_srspp_type;
use crate::transport::srspp::rto::RtoPolicy;
use std::collections::HashMap;
use tokio::time::Duration;
use tokio::time::Instant;

use super::SrsppError;
use super::sleep_until;
use super::ticks_to_duration;

/// Async srspp sender.
///
/// Sends messages reliably over the link, handling segmentation and retransmission.
/// Receives ACKs from the remote receiver.
pub struct SrsppSender<L: NetworkLayer, P: RtoPolicy, const WIN: usize, const BUF: usize, const MTU: usize> {
    link: L,
    rto_policy: P,
    machine: SenderMachine<WIN, BUF, MTU>,
    actions: SenderActions,
    retransmit_timers: HashMap<u16, Instant>,
    ticks_per_sec: u32,
    start_time: Instant,
    recv_buffer: [u8; MTU],
    tx_buffer: [u8; MTU],
}

impl<L: NetworkLayer, P: RtoPolicy, const WIN: usize, const BUF: usize, const MTU: usize>
    SrsppSender<L, P, WIN, BUF, MTU>
{
    /// Create a new sender.
    pub fn new(config: SenderConfig, link: L, rto_policy: P, ticks_per_sec: u32) -> Self {
        Self {
            link,
            rto_policy,
            machine: SenderMachine::new(config),
            actions: SenderActions::new(),
            retransmit_timers: HashMap::new(),
            ticks_per_sec,
            start_time: Instant::now(),
            recv_buffer: [0u8; MTU],
            tx_buffer: [0u8; MTU],
        }
    }

    /// Send a message.
    ///
    /// The message is segmented if necessary and queued for transmission.
    /// This returns when all packets have been transmitted (but not necessarily ACKed).
    ///
    /// For guaranteed delivery, call `flush()` after sending.
    pub async fn send(&mut self, target: Address, data: &(impl IntoBytes + Immutable + ?Sized)) -> Result<(), SrsppError> {
        let data = data.as_bytes();
        self.machine
            .handle(SenderEvent::SendRequest { target, data }, &mut self.actions)?;

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

            result = self.link.recv(&mut self.recv_buffer) => {
                let len = result.map_err(|e| SrsppError::LinkError(e.to_string()))?;
                self.handle_incoming(&self.recv_buffer[..len].to_vec()).await?;
            }

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
        let actions: heapless::Vec<SenderAction, 32> =
            self.actions.iter().copied().collect();

        for action in &actions {
            match action {
                SenderAction::Transmit { seq, .. } => {
                    let cfg = self.machine.config();
                    let source_address = cfg.source_address;
                    let apid = cfg.apid;
                    let function_code = cfg.function_code;
                    let message_id = cfg.message_id;
                    let action_code = cfg.action_code;

                    let packet_len =
                        if let Some(info) = self.machine.get_payload(*seq) {
                            let pkt = SrsppDataPacket::builder()
                                .buffer(&mut self.tx_buffer)
                                .source_address(source_address)
                                .target(info.target)
                                .apid(apid)
                                .function_code(function_code)
                                .message_id(message_id)
                                .action_code(action_code)
                                .sequence_count(*seq)
                                .sequence_flag(info.flags)
                                .payload_len(info.payload.len())
                                .build()
                                .map_err(|e| {
                                    SrsppError::PacketError(
                                        format!("{:?}", e),
                                    )
                                })?;
                            pkt.payload.copy_from_slice(info.payload);
                            Some(
                                SrsppDataPacket::HEADER_SIZE
                                    + info.payload.len(),
                            )
                        } else {
                            None
                        };

                    if let Some(len) = packet_len {
                        self.link
                            .send(&self.tx_buffer[..len])
                            .await
                            .map_err(|e| {
                                SrsppError::LinkError(e.to_string())
                            })?;

                        self.machine.mark_transmitted(*seq);

                        let now = Instant::now();
                        let elapsed = now.duration_since(self.start_time);
                        let now_secs = elapsed.as_secs() as u32;
                        let rto = self.rto_policy.rto_ticks(now_secs);
                        let deadline =
                            now + ticks_to_duration(rto, self.ticks_per_sec);
                        self.retransmit_timers
                            .insert(seq.value(), deadline);
                    }
                }
                SenderAction::StopTimer { seq } => {
                    self.retransmit_timers.remove(&seq.value());
                }
                SenderAction::PacketLost { seq } => {
                    eprintln!(
                        "srspp: Packet {} lost after max retransmits",
                        seq.value()
                    );
                }
                SenderAction::SpaceAvailable { .. } => {}
                SenderAction::MessageLost => {
                    eprintln!("srspp: Segmented message lost");
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
