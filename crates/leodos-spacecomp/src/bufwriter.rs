use core::marker::PhantomData;
use core::mem::size_of;

use zerocopy::Immutable;
use zerocopy::IntoBytes;

use crate::packet::BuildError;
use crate::packet::OpCode;
use crate::packet::SpaceCompMessage;
use crate::node::TxHandle;
use leodos_protocols::network::isl::address::Address;

/// Batched record writer that packs fixed-size records into
/// a SpaceCoMP message buffer and flushes via SRSPP.
pub struct BufWriter<'a, 'tx, T> {
    tx: &'a mut TxHandle<'tx>,
    buf: &'a mut [u8],
    target: Address,
    job_id: u16,
    op_code: OpCode,
    len: usize,
    _record: PhantomData<T>,
}

impl<'a, 'tx, T: IntoBytes + Immutable> BufWriter<'a, 'tx, T> {
    pub fn new(
        tx: &'a mut TxHandle<'tx>,
        buf: &'a mut [u8],
        target: Address,
        job_id: u16,
        op_code: OpCode,
    ) -> Self {
        Self {
            tx,
            buf,
            target,
            job_id,
            op_code,
            len: 0,
            _record: PhantomData,
        }
    }

    fn capacity(&self) -> usize {
        (self.buf.len() - SpaceCompMessage::HEADER_SIZE) / size_of::<T>()
    }

    pub async fn write(&mut self, record: &T) -> Result<(), BuildError> {
        let offset = SpaceCompMessage::HEADER_SIZE + self.len * size_of::<T>();
        self.buf[offset..offset + size_of::<T>()].copy_from_slice(record.as_bytes());
        self.len += 1;

        if self.len >= self.capacity() {
            self.flush().await?;
        }
        Ok(())
    }

    pub async fn flush(&mut self) -> Result<(), BuildError> {
        if self.len == 0 {
            return Ok(());
        }
        let payload_len = self.len * size_of::<T>();
        let msg = SpaceCompMessage::builder()
            .buffer(self.buf)
            .op_code(self.op_code)
            .job_id(self.job_id)
            .payload_len(payload_len)
            .build()?;
        self.tx.send(self.target, msg.as_bytes()).await.ok();
        self.len = 0;
        Ok(())
    }
}
