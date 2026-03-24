//! Standard cFS application scaffolding.
//!
//! Provides an [`App`] builder that automates the common
//! boilerplate every cFS app needs: pipe creation, topic
//! subscription, EVS registration, NoOp/Reset command
//! handling, and housekeeping telemetry.

use crate::cfe::evs::event;
use crate::cfe::sb::msg::{CmdHeader, MessageRef, MsgId, TlmHeader};
use crate::cfe::sb::pipe::Pipe;
use crate::cfe::sb::send_buf::SendBuffer;
use crate::error::Result;

/// Function code for NoOp command.
const FCN_NOOP: u16 = 0;
/// Function code for counter reset command.
const FCN_RESET: u16 = 1;

/// Event ID for NoOp command acknowledgement.
const EVT_NOOP: u16 = 0;
/// Event ID for counter reset acknowledgement.
const EVT_RESET: u16 = 1;
/// Event ID for invalid/unknown command code.
const EVT_INVALID_CC: u16 = 2;
/// Event ID for application startup.
const EVT_STARTUP: u16 = 3;

/// Base housekeeping telemetry payload.
///
/// Contains the standard cmd/err counters that every cFS
/// app reports. Placed after the telemetry header in the
/// HK packet.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct HkTlm {
    /// Total accepted commands since last reset.
    pub cmd_count: u16,
    /// Total rejected commands since last reset.
    pub err_count: u16,
}

/// Standard cFS application with automatic boilerplate.
///
/// Handles pipe creation, topic subscriptions, EVS
/// registration, NoOp (fcn 0), Reset Counters (fcn 1),
/// and housekeeping telemetry publishing.
///
/// # Example
///
/// ```ignore
/// let mut app = App::builder()
///     .name("MY_APP")
///     .cmd_topic(MY_CMD_TOPIC)
///     .send_hk_topic(SEND_HK_TOPIC)
///     .hk_tlm_topic(MY_HK_TLM_TOPIC)
///     .version("1.0.0")
///     .build()?;
///
/// let mut buf = [0u8; 256];
/// loop {
///     let msg = app.recv(&mut buf).await?;
///     match msg.fcn_code()? {
///         2 => {
///             // app-specific command
///             app.ack();
///         }
///         _ => app.reject(msg)?,
///     }
/// }
/// ```
pub struct App {
    pipe: Pipe,
    version: &'static str,
    cmd_msg_id: MsgId,
    send_hk_msg_id: MsgId,
    hk_tlm_msg_id: MsgId,
    cmd_count: u16,
    err_count: u16,
}

#[bon::bon]
impl App {
    /// Creates a new cFS application.
    ///
    /// Registers with EVS, creates a Software Bus pipe,
    /// and subscribes to command and HK wakeup topics.
    ///
    /// Both `cmd_topic` and `send_hk_topic` are treated
    /// as local command topic IDs. `hk_tlm_topic` is a
    /// local telemetry topic ID.
    #[builder]
    pub fn new(
        name: &'static str,
        cmd_topic: u16,
        send_hk_topic: u16,
        hk_tlm_topic: u16,
        version: &'static str,
        #[builder(default = 16)] pipe_depth: u16,
    ) -> Result<Self> {
        event::register(&[])?;

        let pipe = Pipe::new(name, pipe_depth)?;
        let cmd_msg_id = MsgId::local_cmd(cmd_topic);
        let send_hk_msg_id = MsgId::local_cmd(send_hk_topic);
        let hk_tlm_msg_id = MsgId::local_tlm(hk_tlm_topic);

        pipe.subscribe(cmd_msg_id)?;
        pipe.subscribe(send_hk_msg_id)?;

        event::info(EVT_STARTUP, version)?;

        Ok(Self {
            pipe,
            version,
            cmd_msg_id,
            send_hk_msg_id,
            hk_tlm_msg_id,
            cmd_count: 0,
            err_count: 0,
        })
    }
}

impl App {
    /// Processes standard commands (NoOp, Reset, HK) in a
    /// loop. Rejects any unrecognized commands.
    ///
    /// Use this when the app has no custom commands. For
    /// apps that need to handle their own function codes,
    /// use [`recv`](Self::recv) instead.
    pub async fn run(&mut self) -> Result<()> {
        let mut buf = [0u8; 256];
        loop {
            let msg = self.recv(&mut buf).await?;
            self.reject(msg)?;
        }
    }

    /// Receives the next app-specific command.
    ///
    /// Automatically handles:
    /// - Function code 0 (NoOp): info event, increment
    ///   cmd counter
    /// - Function code 1 (Reset): zero both counters,
    ///   info event
    /// - HK wakeup: publish housekeeping telemetry
    ///
    /// Only returns commands the app needs to handle.
    pub async fn recv<'a>(&mut self, buf: &'a mut [u8]) -> Result<MessageRef<'a>> {
        loop {
            let len = self.pipe.recv(buf).await?;
            let msg = MessageRef::new(&buf[..len]);
            let msg_id = msg.msg_id()?;

            if msg_id == self.send_hk_msg_id {
                self.send_hk()?;
                continue;
            }

            if msg_id == self.cmd_msg_id {
                let cmd_hdr_size = core::mem::size_of::<CmdHeader>();
                match msg.fcn_code()? {
                    FCN_NOOP if msg.size()? == cmd_hdr_size => {
                        self.cmd_count = self.cmd_count.wrapping_add(1);
                        event::info(EVT_NOOP, self.version)?;
                    }
                    FCN_RESET if msg.size()? == cmd_hdr_size => {
                        self.cmd_count = 0;
                        self.err_count = 0;
                        event::info(EVT_RESET, "Counters reset")?;
                    }
                    FCN_NOOP | FCN_RESET => {
                        self.err_count = self.err_count.wrapping_add(1);
                        event::error(EVT_INVALID_CC, "Wrong message length")?;
                    }
                    _ => return Ok(MessageRef::new(&buf[..len])),
                }
            }
        }
    }

    /// Acknowledges successful command processing.
    ///
    /// Increments the command counter.
    pub fn ack(&mut self) {
        self.cmd_count = self.cmd_count.wrapping_add(1);
    }

    /// Rejects an unrecognized command.
    ///
    /// Increments the error counter and sends an error
    /// event.
    pub fn reject(&mut self, _msg: MessageRef<'_>) -> Result<()> {
        self.err_count = self.err_count.wrapping_add(1);
        event::error(EVT_INVALID_CC, "Invalid command code")
    }

    /// Publishes base housekeeping telemetry with
    /// cmd_count and err_count.
    fn send_hk(&self) -> Result<()> {
        let hdr = core::mem::size_of::<TlmHeader>();
        let payload = core::mem::size_of::<HkTlm>();
        let size = hdr + payload;

        let mut send_buf = SendBuffer::new(size)?;
        {
            let mut msg = send_buf.view();
            msg.init(self.hk_tlm_msg_id, size)?;
            let hk = msg.payload::<HkTlm>()?;
            hk.cmd_count = self.cmd_count;
            hk.err_count = self.err_count;
            msg.timestamp();
        }
        send_buf.send(true)
    }
}
