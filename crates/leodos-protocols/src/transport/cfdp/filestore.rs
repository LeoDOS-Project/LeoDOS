//! The FileStore trait for abstracting file system operations.
//!
//! This trait allows the CFDP implementation to be generic over any storage
//! backend, from a standard OS file system to an in-memory buffer or a
//! block device driver in a `#![no_std]` environment.

use core::fmt::Debug;
use core::future::Future;

use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::checksum::CfdpChecksum;

/// A unique identifier for an interned file path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(u8);

/// A trait defining the required asynchronous file operations for CFDP.
///
/// The methods return `impl Future` to support backends where I/O is non-blocking,
/// making the trait compatible with any async runtime.
pub trait FileStore: Debug + Send + Sync {
    /// Interns a file path and returns a unique `FileId`.
    fn intern(&self, path: &[u8]) -> Result<FileId, CfdpError>;

    /// Reads a chunk of data from the source file at a given offset.
    ///
    /// # Arguments
    /// * `path`: The path to the source file, as a string slice.
    /// * `offset`: The byte offset from the beginning of the file to start reading.
    /// * `length`: The maximum number of bytes to read.
    /// * `buffer`: The buffer to write the read data into.
    ///
    /// # Returns
    /// A `Future` that resolves to a `Result` containing the number of bytes actually
    /// read. This may be less than `length` if the end of the file is reached.
    fn read_chunk(
        &self,
        path: FileId,
        offset: u64,
        length: u64,
        buffer: &mut [u8],
    ) -> impl Future<Output = Result<usize, CfdpError>>;

    /// Writes a chunk of data to the destination file at a given offset.
    ///
    /// The `FileStore` implementation is responsible for creating the file if it
    /// does not exist.
    ///
    /// # Arguments
    /// * `path`: The path to the destination file, as a string slice.
    /// * `offset`: The byte offset from the beginning of the file to start writing.
    /// * `data`: The slice of data to write.
    ///
    /// # Returns
    /// A `Future` that resolves to a `Result` indicating whether the write was successful.
    fn write_chunk(
        &mut self,
        path: FileId,
        offset: u64,
        data: &[u8],
    ) -> impl Future<Output = Result<(), CfdpError>>;

    /// Retrieves the total size of a file in bytes.
    ///
    /// # Arguments
    /// * `path`: The path to the file, as a string slice.
    ///
    /// # Returns
    /// A `Future` that resolves to a `Result` containing the file size in bytes.
    fn file_size(&self, path: FileId) -> impl Future<Output = Result<u64, CfdpError>>;

    fn calculate_checksum<H>(
        &self,
        file_id: FileId,
        mut hasher: H,
    ) -> impl Future<Output = Result<u32, CfdpError>>
    where
        H: CfdpChecksum,
    {
        async move {
            let file_size = self.file_size(file_id).await?;
            let mut offset: u64 = 0;
            let mut buffer = [0u8; 4096];

            while offset < file_size {
                let remaining = file_size - offset;
                let to_read = core::cmp::min(buffer.len() as u64, remaining) as usize;

                let bytes_read = self
                    .read_chunk(file_id, offset, to_read as u64, &mut buffer)
                    .await?;

                if bytes_read == 0 {
                    break;
                }

                // Update the generic hasher
                hasher.update(&buffer[..bytes_read]);

                offset += bytes_read as u64;
            }

            Ok(hasher.finalize())
        }
    }
}
