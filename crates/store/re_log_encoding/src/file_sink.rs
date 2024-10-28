use std::fmt;
use std::{
    path::PathBuf,
    sync::mpsc::{Receiver, Sender, SyncSender},
};

use parking_lot::Mutex;

use re_log_types::LogMsg;

/// Errors that can occur when creating a [`FileSink`].
#[derive(thiserror::Error, Debug)]
pub enum FileSinkError {
    /// Error creating the file.
    #[error("Failed to create file {0}: {1}")]
    CreateFile(PathBuf, std::io::Error),

    /// Error spawning the file writer thread.
    #[error("Failed to spawn thread: {0}")]
    SpawnThread(std::io::Error),

    /// Error encoding a log message.
    #[error("Failed to encode LogMsg: {0}")]
    LogMsgEncode(#[from] crate::encoder::EncodeError),
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

/// Stream log messages to an `.rrd` file.
pub struct FileSink {
    // None = quit
    tx: Mutex<Sender<Option<Command>>>,
    join_handle: Option<std::thread::JoinHandle<()>>,

    /// Only used for diagnostics, not for access after `new()`.
    ///
    /// `None` indicates stdout.
    path: Option<PathBuf>,
}

impl Drop for FileSink {
    fn drop(&mut self) {
        self.tx.lock().send(None).ok();
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.join().ok();
        }
    }
}

impl FileSink {
    /// Start writing log messages to a file at the given path.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Result<Self, FileSinkError> {
        // We always compress on disk
        let encoding_options = crate::EncodingOptions::COMPRESSED;

        let (tx, rx) = std::sync::mpsc::channel();

        let path = path.into();

        re_log::debug!("Saving file to {path:?}…");

        // TODO(andreas): Can we ensure that a single process doesn't
        // have multiple file sinks for the same file live?
        // This likely caused an instability in the past, see https://github.com/rerun-io/rerun/issues/3306

        let file = std::fs::File::create(&path)
            .map_err(|err| FileSinkError::CreateFile(path.clone(), err))?;
        let encoder = crate::encoder::DroppableEncoder::new(
            re_build_info::CrateVersion::LOCAL,
            encoding_options,
            file,
        )?;
        let join_handle = spawn_and_stream(Some(&path), encoder, rx)?;

        Ok(Self {
            tx: tx.into(),
            join_handle: Some(join_handle),
            path: Some(path),
        })
    }

    /// Start writing log messages to standard output.
    pub fn stdout() -> Result<Self, FileSinkError> {
        let encoding_options = crate::EncodingOptions::COMPRESSED;

        let (tx, rx) = std::sync::mpsc::channel();

        re_log::debug!("Writing to stdout…");

        let encoder = crate::encoder::DroppableEncoder::new(
            re_build_info::CrateVersion::LOCAL,
            encoding_options,
            std::io::stdout(),
        )?;
        let join_handle = spawn_and_stream(None, encoder, rx)?;

        Ok(Self {
            tx: tx.into(),
            join_handle: Some(join_handle),
            path: None,
        })
    }

    #[inline]
    pub fn flush_blocking(&self) {
        let (cmd, oneshot) = Command::flush();
        self.tx.lock().send(Some(cmd)).ok();
        oneshot.recv().ok();
    }

    #[inline]
    pub fn send(&self, log_msg: LogMsg) {
        self.tx.lock().send(Some(Command::Send(log_msg))).ok();
    }
}

/// Set `filepath` to `None` to stream to standard output.
fn spawn_and_stream<W: std::io::Write + Send + 'static>(
    filepath: Option<&std::path::Path>,
    mut encoder: crate::encoder::DroppableEncoder<W>,
    rx: Receiver<Option<Command>>,
) -> Result<std::thread::JoinHandle<()>, FileSinkError> {
    let (name, target) = if let Some(filepath) = filepath {
        ("file_writer", filepath.display().to_string())
    } else {
        ("stdout_writer", "stdout".to_owned())
    };
    std::thread::Builder::new()
        .name(name.into())
        .spawn({
            move || {
                while let Ok(Some(cmd)) = rx.recv() {
                    match cmd {
                        Command::Send(log_msg) => {
                            if let Err(err) = encoder.append(&log_msg) {
                                re_log::error!("Failed to write log stream to {target}: {err}");
                                return;
                            }
                        }
                        Command::Flush(oneshot) => {
                            re_log::trace!("Flushing…");
                            if let Err(err) = encoder.flush_blocking() {
                                re_log::error!("Failed to flush log stream to {target}: {err}");
                                return;
                            }
                            drop(oneshot); // signals the oneshot
                        }
                    }
                }
                if let Err(err) = encoder.finish() {
                    re_log::error!("Failed to end log stream for {target}: {err}");
                    return;
                }
                re_log::debug!("Log stream written to {target}");
            }
        })
        .map_err(FileSinkError::SpawnThread)
}

impl fmt::Debug for FileSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSink")
            .field("path", &self.path.clone().unwrap_or("stdout".into()))
            .finish_non_exhaustive()
    }
}
