use std::{path::PathBuf, sync::mpsc::Sender};

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

/// Stream log messages to an `.rrd` file.
pub struct FileSink {
    // None = quit
    tx: Mutex<Sender<Option<LogMsg>>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
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

        let file = std::fs::File::create(&path)
            .map_err(|err| FileSinkError::CreateFile(path.clone(), err))?;
        let mut encoder = crate::encoder::Encoder::new(encoding_options, file)?;

        let join_handle = std::thread::Builder::new()
            .name("file_writer".into())
            .spawn(move || {
                while let Ok(Some(log_msg)) = rx.recv() {
                    if let Err(err) = encoder.append(&log_msg) {
                        re_log::error!("Failed to save log stream to {path:?}: {err}");
                        return;
                    }
                }
                if let Err(err) = encoder.finish() {
                    re_log::error!("Failed to save log stream to {path:?}: {err}");
                } else {
                    re_log::debug!("Log stream saved to {path:?}");
                }
            })
            .map_err(FileSinkError::SpawnThread)?;

        Ok(Self {
            tx: tx.into(),
            join_handle: Some(join_handle),
        })
    }

    pub fn send(&self, log_msg: LogMsg) {
        self.tx.lock().send(Some(log_msg)).ok();
    }
}
