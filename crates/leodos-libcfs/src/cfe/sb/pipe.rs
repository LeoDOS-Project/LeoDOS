//! Software Bus pipe management for receiving messages.
use crate::cfe::sb::msg::MsgId;
use crate::error::{Error, OsalError, Result};
use crate::ffi::{self, CFE_SB_DEFAULT_QOS};
use crate::status::check;
use bitflags::bitflags;
use core::future::Future;
use core::mem::MaybeUninit;
use core::slice;
use core::task::Poll;
use heapless::{CString, String};

/// A type-safe, zero-cost wrapper for a cFE Software Bus Pipe ID.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct PipeId(pub ffi::CFE_SB_PipeId_t);

impl PartialEq for PipeId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for PipeId {}

impl PipeId {
    /// Converts the Pipe ID into a zero-based integer suitable for array indexing.
    pub fn to_index(&self) -> Result<u32> {
        let mut index = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_SB_PipeId_ToIndex(self.0, index.as_mut_ptr()) })?;
        Ok(unsafe { index.assume_init() })
    }
}

bitflags! {
    /// Options to alter a pipe's behavior.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PipeOptions: u8 {
        /// When set, prevents messages sent from the same application from being
        /// received on this pipe.
        const IGNORE_MINE = ffi::CFE_SB_PIPEOPTS_IGNOREMINE as u8;
    }
}

/// Quality of Service options for a software bus subscription.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Qos(pub(crate) ffi::CFE_SB_Qos_t);

impl Default for Qos {
    fn default() -> Self {
        Self(CFE_SB_DEFAULT_QOS)
    }
}

impl Qos {
    /// Creates new QoS settings.
    pub fn new(priority: u8, reliability: u8) -> Self {
        Self(ffi::CFE_SB_Qos_t {
            Priority: priority,
            Reliability: reliability,
        })
    }

    /// Gets the priority level.
    pub fn priority(&self) -> u8 {
        self.0.Priority
    }

    /// Gets the reliability level.
    pub fn reliability(&self) -> u8 {
        self.0.Reliability
    }
}

/// A CFE Software Bus pipe.
///
/// When dropped, it automatically cleans up the underlying CFE resource.
#[derive(Debug)]
pub struct Pipe {
    id: PipeId,
}

/// Timeout options for receiving messages from a pipe.
pub enum Timeout {
    /// Block indefinitely until a message is received.
    PendForever,
    /// Perform a non-blocking poll for a message.
    Poll,
    /// Wait for the specified number of milliseconds.
    Milliseconds(u32),
}

impl Pipe {
    /// Creates a new software bus pipe.
    ///
    /// # Arguments
    /// * `name` - A unique string to identify the pipe.
    /// * `depth` - The maximum number of messages the pipe can hold.
    pub fn new(name: &str, depth: u16) -> Result<Self> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        let mut pipe_id_uninit = MaybeUninit::<ffi::CFE_SB_PipeId_t>::uninit();
        let status =
            unsafe { ffi::CFE_SB_CreatePipe(pipe_id_uninit.as_mut_ptr(), depth, c_name.as_ptr()) };

        check(status)?;

        let id = PipeId(unsafe { pipe_id_uninit.assume_init() });
        Ok(Self { id })
    }

    /// Subscribes this pipe to messages with the specified `MsgId` and extended options.
    ///
    /// # Arguments
    /// * `msg_id`: The message ID of the message to be subscribed to.
    /// * `qos`: The requested Quality of Service.
    /// * `msg_lim`: The maximum number of messages with this Message ID to
    ///   allow in this pipe at the same time.
    pub fn subscribe_ex(&self, msg_id: MsgId, qos: Qos, msg_lim: u16) -> Result<()> {
        check(unsafe { ffi::CFE_SB_SubscribeEx(msg_id.0, self.id.0, qos.0, msg_lim) })?;
        Ok(())
    }

    /// Subscribes this pipe to messages with the specified `MsgId`.
    ///
    /// Subscriptions are added to the head of an internal linked
    /// list, so messages are delivered in LIFO order (last
    /// subscriber receives first).
    ///
    /// # Arguments
    /// * `msg_id`: The message ID of the message to be subscribed to.
    pub fn subscribe(&self, msg_id: MsgId) -> Result<()> {
        check(unsafe { ffi::CFE_SB_Subscribe(msg_id.0, self.id.0) })?;
        Ok(())
    }

    /// Unsubscribes this pipe from messages with the specified `MsgId`.
    pub fn unsubscribe(&self, msg_id: MsgId) -> Result<()> {
        check(unsafe { ffi::CFE_SB_Unsubscribe(msg_id.0, self.id.0) })?;
        Ok(())
    }

    /// Unsubscribes this pipe from messages, keeping the request local to this CPU.
    ///
    /// This is typically only used by a Software Bus Network (SBN) application.
    pub fn unsubscribe_local(&self, msg_id: MsgId) -> Result<()> {
        check(unsafe { ffi::CFE_SB_UnsubscribeLocal(msg_id.0, self.id.0) })?;
        Ok(())
    }

    /// Receives a message from this pipe, copying it into a user-provided buffer.
    ///
    /// This method receives a message from the CFE-managed internal buffer and safely copies it into the provided `buf`.
    ///
    /// # Arguments
    /// * `timeout`: Timeout in milliseconds. Use `sb::pipe::PEND_FOREVER` to block
    ///   indefinitely or `sb::pipe::POLL` for a non-blocking check.
    /// * `buffer` - A mutable byte slice to copy the message into.
    ///
    /// # Returns
    /// A `MessageRef` containing the message data, tied to the lifetime of `buffer`.
    ///
    /// # Errors
    /// Returns `Error::SbTimeOut` or `Error::SbNoMessage` if no message is received within the timeout.
    /// Returns `Error::SbBadArgument` if the timeout value is invalid.
    /// Returns `Error::OsErrInvalidSize` if the received message is larger than `buf`.
    pub fn timed_recv(&mut self, buf: &mut [u8], timeout: Timeout) -> Result<usize> {
        let mut buf_ptr = MaybeUninit::uninit();

        let timeout = match timeout {
            Timeout::PendForever => ffi::CFE_SB_PEND_FOREVER,
            Timeout::Poll => ffi::CFE_SB_POLL as i32,
            Timeout::Milliseconds(ms) => {
                // Convert to i32, ensuring it fits.
                if ms > i32::MAX as u32 {
                    return Err(Error::CfeSbBadArgument);
                } else {
                    ms as i32
                }
            }
        };
        check(unsafe { ffi::CFE_SB_ReceiveBuffer(buf_ptr.as_mut_ptr(), self.id.0, timeout) })?;

        let buf_ptr = unsafe { buf_ptr.assume_init() };

        let mut size = 0;
        check(unsafe {
            ffi::CFE_MSG_GetSize(buf_ptr as *const ffi::CFE_MSG_Message_t, &mut size)
        })?;

        if size > buf.len() {
            // We must release the buffer back to SB if we can't copy it, to prevent a leak.
            unsafe {
                check(ffi::CFE_SB_ReleaseMessageBuffer(buf_ptr))?;
            }
            return Err(Error::OsErrInvalidSize);
        }

        let src_slice = unsafe { slice::from_raw_parts(buf_ptr as *const u8, size) };
        buf[..size].copy_from_slice(src_slice);

        Ok(size)
    }

    /// Sets options for the pipe, see `PipeOptions` for the available options.
    pub fn set_opts(&self, opts: PipeOptions) -> Result<()> {
        check(unsafe { ffi::CFE_SB_SetPipeOpts(self.id.0, opts.bits()) })?;
        Ok(())
    }

    /// Gets the current options for the pipe, see `PipeOptions` for the available options.
    pub fn get_opts(&self) -> Result<PipeOptions> {
        let mut opts = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_SB_GetPipeOpts(self.id.0, opts.as_mut_ptr()) })?;
        Ok(PipeOptions::from_bits_truncate(unsafe {
            opts.assume_init()
        }))
    }

    /// Returns the underlying `PipeId` for this pipe.
    pub fn id(&self) -> PipeId {
        self.id
    }

    /// Gets the registered name of this pipe.
    pub fn name(&self) -> Result<String<{ ffi::OS_MAX_API_NAME as usize }>> {
        let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
        check(unsafe {
            ffi::CFE_SB_GetPipeName(
                buffer.as_mut_ptr() as *mut libc::c_char,
                buffer.len(),
                self.id.0,
            )
        })?;
        let len = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
        let vec = heapless::Vec::from_slice(&buffer[..len]).map_err(|_| Error::OsErrNameTooLong)?;
        String::from_utf8(vec).map_err(|_| Error::InvalidString)
    }

    /// Finds the `PipeId` for a pipe with the given name.
    pub fn get_id_by_name(name: &str) -> Result<PipeId> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        let mut pipe_id = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_SB_GetPipeIdByName(pipe_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(PipeId(unsafe { pipe_id.assume_init() }))
    }

    /// Subscribes this pipe to messages, keeping the request local to this CPU.
    ///
    /// This is typically only used by a Software Bus Network (SBN) application.
    ///
    /// # Arguments
    /// * `msg_id`: The message ID of the message to be subscribed to.
    /// * `msg_lim`: The maximum number of messages with this Message ID to
    ///   allow in this pipe at the same time.
    pub fn subscribe_local(&self, msg_id: MsgId, msg_lim: u16) -> Result<()> {
        check(unsafe { ffi::CFE_SB_SubscribeLocal(msg_id.0, self.id.0, msg_lim) })?;
        Ok(())
    }
}

impl Drop for Pipe {
    /// Automatically deletes the CFE software bus pipe when the `Pipe` object
    /// goes out of scope.
    fn drop(&mut self) {
        let _ = unsafe { ffi::CFE_SB_DeletePipe(self.id.0) };
    }
}

impl Pipe {
    /// Asynchronously receives a single datagram message on the socket.
    pub fn recv<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<usize>> + use<'a> {
        core::future::poll_fn(|_| {
            let recv_future = self.timed_recv(buf, Timeout::Poll);
            match recv_future {
                Err(Error::Osal(OsalError::Timeout | OsalError::QueueEmpty)) => Poll::Pending,
                Ok(result) => Poll::Ready(Ok(result)),
                Err(e) => Poll::Ready(Err(e)),
            }
        })
    }
}
