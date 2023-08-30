mod config;
mod pipeline;
mod sink;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub use config::{Config, ConfigError};
pub use pipeline::{Pipeline, PipelineError};

#[derive(Default, Clone)]
pub(crate) struct AbortSignal {
    aborted: Arc<AtomicBool>,
}

impl AbortSignal {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn signal(&self) {
        self.aborted.store(true, Ordering::SeqCst);
    }

    pub(crate) fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }
}
