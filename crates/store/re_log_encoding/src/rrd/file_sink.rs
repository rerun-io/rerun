use std::fmt;
use std::path::PathBuf;

use crossbeam::channel::{Receiver, RecvTimeoutError, SendError, Sender};
use parking_lot::Mutex;
use re_log_types::LogMsg;
use re_quota_channel::send_crossbeam;

/// An error that can occur when flushing.
#[derive(Debug, thiserror::Error)]
pub enum FileFlushError {
    #[error("Failed to flush file: {message}")]
    Failed { message: String },

    #[error("File flush timed out - not all messages were written.")]
    Timeout,
}

impl FileFlushError {
    fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }
}

/// Errors that can occur when creating a [`FileSink`].
#[derive(thiserror::Error, Debug)]
pub enum FileSinkError {
    /// Error creating the file.
    #[error("Failed to create file: {source}, path: {path}")]
    CreateFile {
        source: std::io::Error,
        path: PathBuf,
    },

    /// Error spawning the file writer thread.
    #[error("Failed to spawn thread: {0}")]
    SpawnThread(std::io::Error),

    /// Error encoding a log message.
    #[error("Failed to encode LogMsg: {0}")]
    LogMsgEncode(#[from] crate::rrd::EncodeError),
}

#[derive(Debug)]
enum Command {
    Send(LogMsg),
    Flush { on_done: Sender<Result<(), String>> },
}

impl Command {
    fn flush() -> (Self, Receiver<Result<(), String>>) {
        let (tx, rx) = crossbeam::channel::bounded(0); // oneshot
        (Self::Flush { on_done: tx }, rx)
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
        send_crossbeam(&self.tx.lock(), None).ok();
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.join().ok();
        }
    }
}

/// Configuration for a [`FileSink`].
#[derive(Debug, Clone, Copy)]
pub struct FileSinkOptions {
    /// Whether to emit a complete RRD footer (including a manifest of every chunk) at the end
    /// of the stream. Default: `true`.
    ///
    /// To produce a footer, the encoder accumulates per-chunk metadata in memory for the entire
    /// lifetime of the sink. For long-running streaming sessions with many chunks this
    /// grows unboundedly. Set to `false` to opt out: the resulting file is still valid RRD,
    /// but without the manifest which may hurt random-access performance.
    ///
    /// A footer can be added after the fact via `rerun rrd optimize`.
    pub write_footer: bool,
}

impl Default for FileSinkOptions {
    fn default() -> Self {
        Self { write_footer: true }
    }
}

impl FileSink {
    /// Start writing log messages to a file at the given path, with default options.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Result<Self, FileSinkError> {
        Self::with_options(path, FileSinkOptions::default())
    }

    /// Start writing log messages to a file at the given path, with the given [`FileSinkOptions`].
    pub fn with_options(
        path: impl Into<std::path::PathBuf>,
        options: FileSinkOptions,
    ) -> Result<Self, FileSinkError> {
        // We always compress on disk
        let encoding_options = crate::rrd::EncodingOptions::PROTOBUF_COMPRESSED;

        let (tx, rx) = crossbeam::channel::bounded(1024);

        let path = path.into();

        re_log::debug!("Saving file to {path:?}…");

        // TODO(andreas): Can we ensure that a single process doesn't
        // have multiple file sinks for the same file live?
        // This likely caused an instability in the past, see https://github.com/rerun-io/rerun/issues/3306

        let file = std::fs::File::create(&path).map_err(|err| FileSinkError::CreateFile {
            path: path.clone(),
            source: err,
        })?;
        let mut encoder =
            crate::Encoder::new_eager(re_build_info::CrateVersion::LOCAL, encoding_options, file)?;
        if !options.write_footer {
            // The SDK's `FileSink` may stream for the entire lifetime of the host process.
            // The footer's RRD manifest accumulates per-chunk metadata in memory and is only
            // serialized when the encoder is dropped, so leaving it enabled here grows the heap
            // unboundedly (see #12623).
            re_log::warn!(
                "FileSink at {path:?}: `write_footer=false` — the resulting .rrd will not \
                 contain a manifest, which will significantly hurt random-access performance \
                 and some tools (e.g. LazyStore) may not work properly."
            );
            encoder.do_not_emit_footer();
        }
        let join_handle = spawn_and_stream(Some(&path), encoder, rx)?;

        Ok(Self {
            tx: tx.into(),
            join_handle: Some(join_handle),
            path: Some(path),
        })
    }

    /// Start writing log messages to standard output, with default options.
    pub fn stdout() -> Result<Self, FileSinkError> {
        Self::stdout_with_options(FileSinkOptions::default())
    }

    /// Start writing log messages to standard output, with the given [`FileSinkOptions`].
    pub fn stdout_with_options(options: FileSinkOptions) -> Result<Self, FileSinkError> {
        let encoding_options = crate::rrd::EncodingOptions::PROTOBUF_COMPRESSED;

        let (tx, rx) = crossbeam::channel::bounded(1024);

        re_log::debug!("Writing to stdout…");

        let mut encoder = crate::Encoder::new_eager(
            re_build_info::CrateVersion::LOCAL,
            encoding_options,
            std::io::stdout(),
        )?;
        if !options.write_footer {
            // See `Self::with_options` for why we disable footer emission on streaming sinks.
            re_log::warn!(
                "FileSink (stdout): `write_footer=false` — the resulting stream will not \
                 contain a manifest, which will significantly hurt random-access performance \
                 and some tools (e.g. LazyStore) may not work properly."
            );
            encoder.do_not_emit_footer();
        }
        let join_handle = spawn_and_stream(None, encoder, rx)?;

        Ok(Self {
            tx: tx.into(),
            join_handle: Some(join_handle),
            path: None,
        })
    }

    #[inline]
    pub fn flush_blocking(&self, timeout: std::time::Duration) -> Result<(), FileFlushError> {
        let (cmd, oneshot) = Command::flush();
        send_crossbeam(&self.tx.lock(), Some(cmd)).map_err(|_ignored| {
            FileFlushError::failed("File-writer thread shut down prematurely")
        })?;

        oneshot
            .recv_timeout(timeout)
            .map_err(|err| match err {
                RecvTimeoutError::Timeout => FileFlushError::Timeout,
                RecvTimeoutError::Disconnected => {
                    FileFlushError::failed("File-writer thread shut down prematurely")
                }
            })?
            .map_err(FileFlushError::failed)
    }

    #[inline]
    pub fn send(&self, log_msg: LogMsg) {
        send_crossbeam(&self.tx.lock(), Some(Command::Send(log_msg))).ok();
    }
}

/// Set `filepath` to `None` to stream to standard output.
fn spawn_and_stream<W: std::io::Write + Send + 'static>(
    filepath: Option<&std::path::Path>,
    mut encoder: crate::Encoder<W>,
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
                        Command::Flush { on_done } => {
                            re_log::trace!("Flushing…");

                            let result = encoder.flush_blocking().map_err(|err| {
                                format!("Failed to flush log stream to {target}: {err}")
                            });

                            // Send back the result:
                            if let Err(SendError(result)) = send_crossbeam(&on_done, result)
                                && let Err(err) = result
                            {
                                // There was an error, and nobody received it:
                                re_log::error!("{err}");
                            }
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
            .field(
                "path",
                &self.path.clone().unwrap_or_else(|| "stdout".into()),
            )
            .finish_non_exhaustive()
    }
}
