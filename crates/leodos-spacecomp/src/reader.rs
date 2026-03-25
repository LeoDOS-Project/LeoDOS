use core::marker::PhantomData;
use core::mem::size_of;

use zerocopy::{FromBytes, Immutable, KnownLayout};

/// Iterator over fixed-size records in a byte slice.
pub struct RecordIter<'a, T> {
    data: &'a [u8],
    offset: usize,
    _record: PhantomData<T>,
}

impl<'a, T: FromBytes + Immutable + KnownLayout + 'a> RecordIter<'a, T> {
    /// Creates a new iterator over `T` records in `data`.
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            offset: 0,
            _record: PhantomData,
        }
    }
}

impl<'a, T: FromBytes + Immutable + KnownLayout + 'a> Iterator for RecordIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let end = self.offset + size_of::<T>();
        if end > self.data.len() {
            return None;
        }
        let record = T::ref_from_bytes(&self.data[self.offset..end]).ok()?;
        self.offset = end;
        Some(record)
    }
}
