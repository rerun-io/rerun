use std::io;

/// Asynchronous positional reads of bytes.
///
/// Reads are stateless (`&self`, explicit `offset`) and return owned [`bytes::Bytes`], so a single
/// reader can serve concurrent reads without a shared cursor, and an in-memory reader can hand back
/// zero-copy slices of its backing buffer.
//
// TODO(grtlr): `std::fs::File::read_exact_at` performs blocking I/O on the async executor thread.
// Run the complete positioned read via `spawn_blocking`.
#[async_trait::async_trait]
pub trait AsyncReadAt: Send + Sync {
    /// Reads exactly `len` bytes starting at `offset`.
    ///
    /// Returns [`io::ErrorKind::UnexpectedEof`] if the stream ends before `len` bytes are read.
    async fn read_exact_at(&self, offset: u64, len: usize) -> io::Result<bytes::Bytes>;

    /// Returns the total number of bytes available.
    async fn size(&self) -> io::Result<u64>;
}

/// Blocking positional reads backed by the OS. `pread`/`seek_read` do not use the file cursor,
/// so `&self` reads are safe to issue concurrently.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl AsyncReadAt for std::fs::File {
    async fn read_exact_at(&self, offset: u64, len: usize) -> io::Result<bytes::Bytes> {
        let mut buf = vec![0u8; len];
        let mut filled = 0;
        while filled < len {
            let n = {
                #[cfg(unix)]
                {
                    std::os::unix::fs::FileExt::read_at(
                        self,
                        &mut buf[filled..],
                        offset + filled as u64,
                    )?
                }
                #[cfg(windows)]
                {
                    std::os::windows::fs::FileExt::seek_read(
                        self,
                        &mut buf[filled..],
                        offset + filled as u64,
                    )?
                }
            };
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "failed to fill whole buffer",
                ));
            }
            filled += n;
        }
        Ok(bytes::Bytes::from(buf))
    }

    async fn size(&self) -> io::Result<u64> {
        Ok(self.metadata()?.len())
    }
}

/// In-memory positional reads.
///
/// [`bytes::Bytes::slice`] shares the backing allocation, so reads are zero-copy.
#[async_trait::async_trait]
impl AsyncReadAt for bytes::Bytes {
    async fn read_exact_at(&self, offset: u64, len: usize) -> io::Result<Self> {
        let Ok(start) = usize::try_from(offset) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "offset out of range",
            ));
        };
        let end = start
            .checked_add(len)
            .filter(|&end| end <= self.len())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "read past end of buffer")
            })?;
        Ok(self.slice(start..end))
    }

    async fn size(&self) -> io::Result<u64> {
        Ok(self.len() as u64)
    }
}
