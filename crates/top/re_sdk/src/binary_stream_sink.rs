use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::sync::Arc;

use parking_lot::Mutex;

use re_log_encoding::encoder::{encode_as_bytes_local, encode_ref_as_bytes_local};
use re_log_types::LogMsg;

use crate::sink::LogSink;
use crate::RecordingStream;

/// Errors that can occur when creating a [`BinaryStreamSink`].
#[derive(thiserror::Error, Debug)]
pub enum BinaryStreamSinkError {
    /// Error spawning the writer thread.
    #[error("Failed to spawn thread: {0}")]
    SpawnThread(std::io::Error),

    /// Error encoding a log message.
    #[error("Failed to encode LogMsg: {0}")]
    LogMsgEncode(#[from] re_log_encoding::encoder::EncodeError),
}

enum Command {
    Send(LogMsg),
    Flush(SyncSender<()>),
}

impl Command {
    fn flush() -> (Self, Receiver<()>) {
        let (tx, rx) = std::sync::mpsc::sync_channel(0); // oneshot
        (Self::Flush(tx), rx)
    }
}

/// The inner storage used by [`BinaryStreamStorage`].
///
/// Although this implements Clone so that it can be shared between the encoder thread and the outer
/// storage, the model is that reading from it consumes the buffer.
#[derive(Clone, Default)]
struct BinaryStreamStorageInner(Arc<Mutex<std::io::Cursor<Vec<u8>>>>);

impl std::io::Write for BinaryStreamStorageInner {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().flush()
    }
}

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
    /// This does not flush the underlying batcher.
    /// Use [`BinaryStreamStorage::flush`] if you want to guarantee that all
    /// logged messages have been written to the stream before you read them.
    #[inline]
    pub fn read(&self) -> Vec<u8> {
        // TODO(jan, gijs): handle error here?
        encode_as_bytes_local(self.inner.lock().drain(..).map(Ok)).unwrap_or_default()
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
    pub fn new(rec: RecordingStream) -> Result<(Self, BinaryStreamStorage), BinaryStreamSinkError> {
        let storage = BinaryStreamStorage::new(rec);

        Ok((
            Self {
                buffer: storage.inner.clone(),
            },
            storage,
        ))
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
