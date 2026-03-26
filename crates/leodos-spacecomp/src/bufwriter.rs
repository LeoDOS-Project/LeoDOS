use core::marker::PhantomData;
use core::mem::size_of;

use zerocopy::Immutable;
use zerocopy::IntoBytes;

use crate::transport::Tx;
use crate::SpaceCompError;

/// Batched record writer. Packs fixed-size records into a
/// buffer and flushes via [`Tx::send`] when full.
pub struct BufWriter<'a, T, S: Tx> {
    tx: &'a mut S,
    buf: [u8; 4096],
    len: usize,
    _record: PhantomData<T>,
}

impl<'a, T: IntoBytes + Immutable, S: Tx> BufWriter<'a, T, S> {
    pub fn new(tx: &'a mut S) -> Self {
        Self {
            tx,
            buf: [0u8; 4096],
            len: 0,
            _record: PhantomData,
        }
    }

    fn capacity(&self) -> usize {
        self.buf.len() / size_of::<T>()
    }

    pub async fn write(&mut self, record: &T) -> Result<(), SpaceCompError> {
        let offset = self.len * size_of::<T>();
        self.buf[offset..offset + size_of::<T>()].copy_from_slice(record.as_bytes());
        self.len += 1;

        if self.len >= self.capacity() {
            self.flush().await?;
        }
        Ok(())
    }

    pub async fn flush(&mut self) -> Result<(), SpaceCompError> {
        if self.len == 0 {
            return Ok(());
        }
        let payload_len = self.len * size_of::<T>();
        self.tx.send(&self.buf[..payload_len]).await?;
        self.len = 0;
        Ok(())
    }
}
