//! Have multiple loggers implementing [`log::Log`] at once.

use std::sync::atomic::{AtomicBool, Ordering::SeqCst};

static MULTI_LOGGER: MultiLogger = MultiLogger::new();

static HAS_MULTI_LOGGER: AtomicBool = AtomicBool::new(false);

/// Install the multi-logger as the default logger.
pub fn init() -> Result<(), log::SetLoggerError> {
    HAS_MULTI_LOGGER.store(true, SeqCst);
    log::set_logger(&MULTI_LOGGER)
}

/// Install an additional global logger.
pub fn add_boxed_logger(logger: Box<dyn log::Log>) {
    add_logger(Box::leak(logger));
}

/// Install an additional global logger.
pub fn add_logger(logger: &'static dyn log::Log) {
    debug_assert!(
        HAS_MULTI_LOGGER.load(SeqCst),
        "You forgot to setup multi-logging"
    );
    MULTI_LOGGER.loggers.write().push(logger);
}

/// Forward log messages to multiple [`log::log`] receivers.
struct MultiLogger {
    loggers: parking_lot::RwLock<Vec<&'static dyn log::Log>>,
}

impl MultiLogger {
    pub const fn new() -> Self {
        Self {
            loggers: parking_lot::RwLock::new(vec![]),
        }
    }
}

impl log::Log for MultiLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.loggers
            .read()
            .iter()
            .any(|logger| logger.enabled(metadata))
    }

    fn log(&self, record: &log::Record<'_>) {
        for logger in self.loggers.read().iter() {
            logger.log(record);
        }
    }

    fn flush(&self) {
        for logger in self.loggers.read().iter() {
            logger.flush();
        }
    }
}
