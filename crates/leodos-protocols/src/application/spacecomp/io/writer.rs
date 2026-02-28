use core::future::Future;
use core::marker::PhantomData;
use core::mem::size_of;

use zerocopy::{Immutable, IntoBytes};

use crate::application::spacecomp::packet::{BuildError, OpCode, SpaceCompMessage};
use crate::network::isl::address::Address;

/// Addressed message sender for SpaceCoMP communication.
pub trait MessageSender {
    /// Error type returned by send operations.
    type Error;

    /// Sends a raw message to the given target address.
    fn send_message(
        &mut self,
        target: Address,
        data: &[u8],
    ) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Batched record writer that packs fixed-size records into
/// SpaceCoMP messages and flushes them via a [`MessageSender`].
pub struct BufWriter<'a, T, S> {
    sender: &'a mut S,
    msg_buf: &'a mut [u8],
    payload_buf: &'a mut [u8],
    target: Address,
    job_id: u16,
    op_code: OpCode,
    len: usize,
    _record: PhantomData<T>,
}

impl<'a, T: IntoBytes + Immutable, S: MessageSender> BufWriter<'a, T, S> {
    /// Creates a new writer that batches records of type `T`.
    pub fn new(
        sender: &'a mut S,
        msg_buf: &'a mut [u8],
        payload_buf: &'a mut [u8],
        target: Address,
        job_id: u16,
        op_code: OpCode,
    ) -> Self {
        Self {
            sender,
            msg_buf,
            payload_buf,
            target,
            job_id,
            op_code,
            len: 0,
            _record: PhantomData,
        }
    }

    fn capacity(&self) -> usize {
        self.payload_buf.len() / size_of::<T>()
    }

    /// Buffers a record, flushing automatically when full.
    pub async fn write(&mut self, record: &T) -> Result<(), BuildError> {
        let offset = self.len * size_of::<T>();
        self.payload_buf[offset..offset + size_of::<T>()]
            .copy_from_slice(record.as_bytes());
        self.len += 1;

        if self.len >= self.capacity() {
            self.flush().await?;
        }
        Ok(())
    }

    /// Sends any buffered records as a SpaceCoMP message.
    pub async fn flush(&mut self) -> Result<(), BuildError> {
        if self.len == 0 {
            return Ok(());
        }
        let payload_len = self.len * size_of::<T>();
        let msg = SpaceCompMessage::builder()
            .buffer(self.msg_buf)
            .op_code(self.op_code)
            .job_id(self.job_id)
            .payload(&self.payload_buf[..payload_len])
            .build()?;
        self.sender.send_message(self.target, msg.as_bytes()).await.ok();
        self.len = 0;
        Ok(())
    }
}
