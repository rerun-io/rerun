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
        re_tracing::profile_function!();

        let offset = span.start;
        let len = span_len_usize(span)?;

        // Spans come from an on-disk manifest, so check them against the file before allocating.
        let file_len = self.metadata()?.len();
        if span
            .start
            .checked_add(span.len)
            .is_none_or(|end| end > file_len)
        {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "read past end of file",
            ));
        }
        let mut buf = Vec::new();
        buf.try_reserve_exact(len)
            .map_err(|err| io::Error::new(io::ErrorKind::OutOfMemory, err))?;
        buf.resize(len, 0);
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::io::Write as _;

    use super::{AsyncReadAt as _, Span};

    /// A span past the end of the file must fail before anything is allocated for it.
    #[test]
    fn file_read_past_eof_is_rejected() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"hello").unwrap();
        let file = std::fs::File::open(tmp.path()).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread() // NOLINT: the test owns this runtime
            .build()
            .unwrap();

        for span in [
            Span {
                start: 0,
                len: u64::MAX,
            },
            Span { start: 3, len: 3 },
            Span {
                start: u64::MAX,
                len: 1,
            },
        ] {
            let err = rt.block_on(file.read_exact_at(span)).unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof, "{span:?}");
        }

        let ok = rt
            .block_on(file.read_exact_at(Span { start: 1, len: 3 }))
            .unwrap();
        assert_eq!(&ok[..], b"ell");
    }
}
