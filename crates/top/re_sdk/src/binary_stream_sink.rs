use std::sync::Arc;

use parking_lot::Mutex;
use re_log::ResultExt as _;
use re_log_encoding::Encoder;
use re_log_types::LogMsg;

use crate::RecordingStream;
use crate::log_sink::SinkFlushError;
use crate::sink::LogSink;

/// The storage used by [`BinaryStreamSink`].
///
/// Reading from this consumes the bytes from the stream.
pub struct BinaryStreamStorage {
    inner: Arc<Mutex<Vec<LogMsg>>>,
    rec: RecordingStream,
}

impl BinaryStreamStorage {
    /// Create a new binary stream storage.
    fn new(rec: RecordingStream) -> Self {
        Self {
            inner: Default::default(),
            rec,
        }
    }

    /// Read and consume the current contents of the buffer.
    ///
    /// This returns a fully encoded RRD file.
    ///
    /// This does not flush the underlying batcher.
    /// Use [`BinaryStreamStorage::flush`] if you want to guarantee that all
    /// logged messages have been written to the stream before you read them.
    #[inline]
    pub fn read(&self) -> Option<Vec<u8>> {
        let mut inner = self.inner.lock();

        // if there's no messages to send, do not include the RRD headers.
        if inner.is_empty() {
            return None;
        }

        Encoder::encode(inner.drain(..).map(Ok)).ok_or_log_error()
    }

    /// Flush the batcher and log encoder to guarantee that all logged messages
    /// have been written to the stream.
    #[inline]
    pub fn flush(&self, timeout: std::time::Duration) -> Result<(), SinkFlushError> {
        self.rec.flush_with_timeout(timeout)
    }
}

impl Drop for BinaryStreamStorage {
    fn drop(&mut self) {
        if let Err(err) = self.flush(std::time::Duration::MAX) {
            re_log::error!("Failed to flush BinaryStreamStorage: {err}");
        }

        let bytes = self.read();

        if let Some(bytes) = bytes {
            re_log::warn!(
                "Dropping data in BinaryStreamStorage ({} bytes)",
                bytes.len()
            );
        }
    }
}

/// Stream log messages to an in-memory binary stream.
///
/// The contents of this stream are encoded in the Rerun Record Data format (rrd).
///
/// This stream has no mechanism of limiting memory or creating back-pressure. If you do not
/// read from it, it will buffer all messages that you have logged.
pub struct BinaryStreamSink {
    buffer: Arc<Mutex<Vec<LogMsg>>>,
}

impl BinaryStreamSink {
    /// Create a pair of a new [`BinaryStreamSink`] and the associated [`BinaryStreamStorage`].
    pub fn new(rec: RecordingStream) -> (Self, BinaryStreamStorage) {
        let storage = BinaryStreamStorage::new(rec);

        (
            Self {
                buffer: storage.inner.clone(),
            },
            storage,
        )
    }
}

impl LogSink for BinaryStreamSink {
    #[inline]
    fn send(&self, msg: re_log_types::LogMsg) {
        self.buffer.lock().push(msg);
    }

    #[inline]
    fn flush_blocking(&self, _timeout: std::time::Duration) -> Result<(), SinkFlushError> {
        Ok(())
    }
}
