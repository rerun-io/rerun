use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::sync::Arc;

use parking_lot::Mutex;

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
    inner: BinaryStreamStorageInner,
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
        let mut buffer = std::io::Cursor::new(Vec::new());
        std::mem::swap(&mut buffer, &mut *self.inner.0.lock());
        buffer.into_inner()
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
    /// The sender to the encoder thread.
    tx: Mutex<Sender<Option<Command>>>,

    /// Handle to join the encoder thread on drop.
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for BinaryStreamSink {
    fn drop(&mut self) {
        self.tx.lock().send(None).ok();
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.join().ok();
        }
    }
}

impl BinaryStreamSink {
    /// Create a pair of a new [`BinaryStreamSink`] and the associated [`BinaryStreamStorage`].
    pub fn new(rec: RecordingStream) -> Result<(Self, BinaryStreamStorage), BinaryStreamSinkError> {
        let storage = BinaryStreamStorage::new(rec);

        // We always compress when writing to a stream
        // TODO(jleibs): Make this configurable
        let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;

        let (tx, rx) = std::sync::mpsc::channel();

        let encoder = re_log_encoding::encoder::DroppableEncoder::new(
            re_build_info::CrateVersion::LOCAL,
            encoding_options,
            storage.inner.clone(),
        )?;

        let join_handle = spawn_and_stream(encoder, rx)?;

        Ok((
            Self {
                tx: tx.into(),
                join_handle: Some(join_handle),
            },
            storage,
        ))
    }
}

impl LogSink for BinaryStreamSink {
    #[inline]
    fn send(&self, msg: re_log_types::LogMsg) {
        self.tx.lock().send(Some(Command::Send(msg))).ok();
    }

    #[inline]
    fn flush_blocking(&self) {
        let (cmd, oneshot) = Command::flush();
        self.tx.lock().send(Some(cmd)).ok();
        oneshot.recv().ok();
    }
}

/// Spawn the encoder thread that will write log messages to the binary stream.
fn spawn_and_stream<W: std::io::Write + Send + 'static>(
    mut encoder: re_log_encoding::encoder::DroppableEncoder<W>,
    rx: Receiver<Option<Command>>,
) -> Result<std::thread::JoinHandle<()>, BinaryStreamSinkError> {
    std::thread::Builder::new()
        .name("binary_stream_encoder".into())
        .spawn({
            move || {
                while let Ok(Some(cmd)) = rx.recv() {
                    match cmd {
                        Command::Send(log_msg) => {
                            if let Err(err) = encoder.append(&log_msg) {
                                re_log::error!(
                                    "Failed to write log stream to binary stream: {err}"
                                );
                                return;
                            }
                        }
                        Command::Flush(oneshot) => {
                            re_log::trace!("Flushing…");
                            if let Err(err) = encoder.flush_blocking() {
                                re_log::error!(
                                    "Failed to flush log stream to binary stream: {err}"
                                );
                                return;
                            }
                            drop(oneshot); // signals the oneshot
                        }
                    }
                }
                re_log::debug!("Log stream written to binary stream");
            }
        })
        .map_err(BinaryStreamSinkError::SpawnThread)
}
