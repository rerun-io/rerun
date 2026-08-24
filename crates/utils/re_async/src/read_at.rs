use std::io;

use re_span::Span;

/// Asynchronous positional reads of bytes.
///
/// Reads are stateless (`&self`, explicit `offset`) and return owned [`bytes::Bytes`], so a single
/// reader can serve concurrent reads without a shared cursor, and an in-memory reader can hand back
/// zero-copy slices of its backing buffer.
//
// TODO(grtlr): `std::fs::File::read_exact_at` performs blocking I/O on the async executor thread.
// Run the complete positioned read via `spawn_blocking`.
/// Convert `span.len` for indexing, failing where it does not fit
/// (`usize` is 32-bit on wasm).
///
/// Shared by [`AsyncReadAt`] implementations so the error stays uniform.
pub fn span_len_usize(span: Span<u64>) -> io::Result<usize> {
    usize::try_from(span.len).map_err(|_err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "read length does not fit this platform's pointer size",
        )
    })
}

#[async_trait::async_trait]
pub trait AsyncReadAt: Send + Sync {
    /// Reads exactly the bytes of `span`.
    ///
    /// Returns [`io::ErrorKind::UnexpectedEof`] if the stream ends before `span.len` bytes are read.
    async fn read_exact_at(&self, span: Span<u64>) -> io::Result<bytes::Bytes>;

    /// Returns the total number of bytes available.
    async fn size(&self) -> io::Result<u64>;
}

/// Blocking positional reads backed by the OS. `pread`/`seek_read` do not use the file cursor,
/// so `&self` reads are safe to issue concurrently.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl AsyncReadAt for std::fs::File {
    async fn read_exact_at(&self, span: Span<u64>) -> io::Result<bytes::Bytes> {
        let offset = span.start;
        let len = span_len_usize(span)?;
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
    async fn read_exact_at(&self, span: Span<u64>) -> io::Result<Self> {
        let start = usize::try_from(span.start)
            .map_err(|_err| io::Error::new(io::ErrorKind::InvalidInput, "span out of range"))?;
        let len = span_len_usize(span)?;
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
