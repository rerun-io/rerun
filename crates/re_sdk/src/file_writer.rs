use anyhow::Context as _;

use re_log_types::LogMsg;

pub struct FileWriter {
    // None = quit
    tx: std::sync::mpsc::Sender<Option<LogMsg>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for FileWriter {
    fn drop(&mut self) {
        self.tx.send(None).ok();
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.join().ok();
        }
    }
}

impl FileWriter {
    pub fn new(path: impl Into<std::path::PathBuf>) -> anyhow::Result<Self> {
        let (tx, rx) = std::sync::mpsc::channel();

        let path = path.into();

        re_log::debug!("Saving file to {path:?}…");

        let file = std::fs::File::create(&path).with_context(|| format!("Path: {:?}", path))?;
        let mut encoder = re_log_types::encoding::Encoder::new(file)?;

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
            .context("Failed to spawn thread")?;

        Ok(Self {
            tx,
            join_handle: Some(join_handle),
        })
    }

    pub fn write(&self, msg: LogMsg) {
        self.tx.send(Some(msg)).ok();
    }
}
