//! Capture log messages and send them to some receiver over a channel.

pub use crossbeam::channel::{Receiver, Sender};

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
    tx: parking_lot::Mutex<Sender<LogMsg>>,
}

impl ChannelLogger {
    pub fn new(filter: log::LevelFilter) -> (Self, Receiver<LogMsg>) {
        // can't block on web, so we cannot apply backpressure
        #[cfg_attr(not(target_arch = "wasm32"), expect(clippy::disallowed_methods))]
        let (tx, rx) = crossbeam::channel::unbounded();
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
