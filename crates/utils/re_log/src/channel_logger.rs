//! Capture log messages and send them to some receiver over a channel.

#[derive(Clone)]
pub struct LogMsg {
    /// The verbosity level.
    pub level: log::Level,

    /// The module, starting with the crate name.
    pub target: String,

    /// The contents of the log message.
    pub msg: String,
}

/// Pipe log messages to a channel.
pub struct ChannelLogger {
    filter: log::LevelFilter,
    tx: parking_lot::Mutex<std::sync::mpsc::Sender<LogMsg>>,
}

impl ChannelLogger {
    pub fn new(filter: log::LevelFilter) -> (Self, std::sync::mpsc::Receiver<LogMsg>) {
        let (tx, rx) = std::sync::mpsc::channel();
        (
            Self {
                filter,
                tx: tx.into(),
            },
            rx,
        )
    }
}

impl log::Log for ChannelLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        crate::is_log_enabled(self.filter, metadata)
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        self.tx
            .lock()
            .send(LogMsg {
                level: record.level(),
                target: record.target().to_owned(),
                msg: record.args().to_string(),
            })
            .ok();
    }

    fn flush(&self) {}
}
