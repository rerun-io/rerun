//! Capture log messages and send them to some receiver over a channel.

pub struct LogMsg {
    pub level: log::Level,
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
                msg: record.args().to_string(),
            })
            .ok();
    }

    fn flush(&self) {}
}
