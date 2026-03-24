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

/// Event returned by [`App::recv`].
pub enum Event<'a> {
    /// An app-specific command (NoOp/Reset already handled).
    Command(MessageRef<'a>),
    /// HK wakeup — the app should publish telemetry.
    Hk,
}

/// Standard cFS application with automatic boilerplate.
///
/// Handles pipe creation, topic subscriptions, EVS
/// registration, NoOp (fcn 0), and Reset Counters (fcn 1).
///
/// HK wakeups are surfaced as [`Event::Hk`] so the app
/// controls what telemetry to publish.
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
///     match app.recv(&mut buf).await? {
///         Event::Hk => app.send_hk(&my_hk)?,
///         Event::Command(msg) => app.reject(msg)?,
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
    /// Processes commands and HK in a loop.
    ///
    /// Publishes base HK (cmd/err counters) on wakeup,
    /// rejects unrecognized commands. For custom HK or
    /// app-specific commands, use [`recv`](Self::recv).
    pub async fn run(&mut self) -> Result<()> {
        let mut buf = [0u8; 256];
        loop {
            match self.recv(&mut buf).await? {
                Event::Hk => self.send_hk_base()?,
                Event::Command(msg) => self.reject(msg)?,
            }
        }
    }

    /// Receives the next event from the Software Bus.
    ///
    /// Automatically handles NoOp (fcn 0) and Reset (fcn 1).
    /// Returns [`Event::Hk`] on HK wakeup so the app can
    /// publish its own telemetry. Returns [`Event::Command`]
    /// for app-specific commands.
    pub async fn recv<'a>(&mut self, buf: &'a mut [u8]) -> Result<Event<'a>> {
        loop {
            let len = self.pipe.recv(buf).await?;
            let msg = MessageRef::new(&buf[..len]);
            let msg_id = msg.msg_id()?;

            if msg_id == self.send_hk_msg_id {
                return Ok(Event::Hk);
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
                    _ => return Ok(Event::Command(MessageRef::new(&buf[..len]))),
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

    /// Publishes a custom HK telemetry packet.
    ///
    /// The payload is serialized via `as_bytes()`. The app
    /// should include cmd/err counters (from
    /// [`cmd_count`](Self::cmd_count) /
    /// [`err_count`](Self::err_count)) in the struct.
    pub fn send_hk<H: Copy>(&self, payload: &H) -> Result<()> {
        let hdr = core::mem::size_of::<TlmHeader>();
        let size = hdr + core::mem::size_of::<H>();

        let mut send_buf = SendBuffer::new(size)?;
        {
            let mut msg = send_buf.view();
            msg.init(self.hk_tlm_msg_id, size)?;
            *msg.payload::<H>()? = *payload;
            msg.timestamp();
        }
        send_buf.send(true)
    }

    /// Returns the current command counter.
    pub fn cmd_count(&self) -> u16 {
        self.cmd_count
    }

    /// Returns the current error counter.
    pub fn err_count(&self) -> u16 {
        self.err_count
    }

    /// Publishes base HK (cmd/err counters only).
    fn send_hk_base(&self) -> Result<()> {
        self.send_hk(&HkTlm {
            cmd_count: self.cmd_count,
            err_count: self.err_count,
        })
    }
}
