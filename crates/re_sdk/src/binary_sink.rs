use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::sync::Arc;

use parking_lot::Mutex;

use re_log_types::LogMsg;

use crate::sink::LogSink;
use crate::RecordingStream;

/// Errors that can occur when creating a [`FileSink`].
#[derive(thiserror::Error, Debug)]
pub enum BinarySinkError {
    /// Error spawning the file writer thread.
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

#[derive(Clone)]
pub struct BinarySinkStorage {
    inner: Arc<Mutex<std::io::Cursor<Vec<u8>>>>,
    pub(crate) rec: RecordingStream,
}

impl BinarySinkStorage {
    fn new(rec: RecordingStream) -> Self {
        Self {
            inner: Default::default(),
            rec,
        }
    }

    #[inline]
    pub fn read(&self) -> Vec<u8> {
        self.rec.flush_blocking();
        let mut buffer = std::io::Cursor::new(Vec::new());
        std::mem::swap(&mut buffer, &mut *self.inner.lock());
        buffer.into_inner()
    }
}

impl std::io::Write for BinarySinkStorage {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.lock().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.lock().flush()
    }
}

/// Stream log messages to an in-memory binary stream.
pub struct BinarySink {
    // None = quit
    tx: Mutex<Sender<Option<Command>>>,
    join_handle: Option<std::thread::JoinHandle<()>>,

    storage: BinarySinkStorage,
}

impl Drop for BinarySink {
    fn drop(&mut self) {
        self.tx.lock().send(None).ok();
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.join().ok();
        }
    }
}

impl BinarySink {
    /// Start writing log messages to a file at the given path.
    pub fn new(rec: RecordingStream) -> Result<Self, BinarySinkError> {
        let storage = BinarySinkStorage::new(rec);

        // We always compress when writing to a stream
        let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;

        let (tx, rx) = std::sync::mpsc::channel();

        let encoder = re_log_encoding::encoder::Encoder::new(encoding_options, storage.clone())?;

        let join_handle = spawn_and_stream(encoder, rx)?;

        Ok(Self {
            tx: tx.into(),
            join_handle: Some(join_handle),
            storage,
        })
    }

    /// Access the raw `BinarySinkStorage`
    #[inline]
    pub fn buffer(&self) -> BinarySinkStorage {
        self.storage.clone()
    }
}

impl LogSink for BinarySink {
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

/// Set `filepath` to `None` to stream to standard output.
fn spawn_and_stream<W: std::io::Write + Send + 'static>(
    mut encoder: re_log_encoding::encoder::Encoder<W>,
    rx: Receiver<Option<Command>>,
) -> Result<std::thread::JoinHandle<()>, BinarySinkError> {
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
        .map_err(BinarySinkError::SpawnThread)
}
