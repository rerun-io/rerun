use std::sync::Arc;

use parking_lot::Mutex;

use re_log::ResultExt;
use re_log_encoding::encoder::encode_as_bytes_local;
use re_log_types::LogMsg;

use crate::sink::LogSink;
use crate::RecordingStream;

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
    pub fn read(&self) -> Vec<u8> {
        encode_as_bytes_local(self.inner.lock().drain(..).map(Ok))
            .ok_or_log_error()
            .unwrap_or_default()
    }

    /// Flush the batcher and log encoder to guarantee that all logged messages
    /// have been written to the stream.
    ///
    /// This will block until the flush is complete.
    #[inline]
    pub fn flush(&self) {
        self.rec.flush_blocking();
    }
}

impl Drop for BinaryStreamStorage {
    fn drop(&mut self) {
        self.flush();
        let bytes = self.read();

        if !bytes.is_empty() {
            re_log::warn!("Dropping data in BinaryStreamStorage");
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
    fn flush_blocking(&self) {}
}
