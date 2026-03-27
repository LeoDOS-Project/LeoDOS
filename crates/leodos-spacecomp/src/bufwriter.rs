use core::marker::PhantomData;
use core::mem::size_of;

use zerocopy::Immutable;
use zerocopy::IntoBytes;

use crate::transport::Tx;
use crate::SpaceCompError;

/// Batched record writer. Packs fixed-size records into a
/// buffer and flushes via [`Tx::send`] when full.
///
/// ## Note
///
/// The buffer should be flushed before it is dropped to
/// avoid losing unflushed records.
pub struct BufWriter<'a, 'b, T, S: Tx> {
    tx: &'a mut S,
    buf: &'b mut [u8],
    len: usize,
    _record: PhantomData<T>,
}

impl<'a, 'b, T: IntoBytes + Immutable, S: Tx> BufWriter<'a, 'b, T, S> {
    pub fn new(tx: &'a mut S, buf: &'b mut [u8]) -> Self {
        Self {
            tx,
            buf,
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

impl<T, S: Tx> Drop for BufWriter<'_, '_, T, S> {
    fn drop(&mut self) {
        debug_assert!(
            self.len == 0,
            "BufWriter dropped with {} unflushed records",
            self.len
        );
    }
}
