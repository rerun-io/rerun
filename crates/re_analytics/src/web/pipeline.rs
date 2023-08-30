#![allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    clippy::unused_self
)]

use std::time::Duration;

use super::sink::PostHogSink;
use crate::{Config, Event};

// TODO(cmc): abstract away the concept of a `Pipeline` behind an actual trait when comes the time
// to support more than just PostHog.

// ---

#[derive(thiserror::Error, Debug)]
pub enum PipelineError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

/// An eventual, at-least-once(-ish) event pipeline, backed by a write-ahead log on the local disk.
///
/// Flushing of the WAL is entirely left up to the OS page cache, hance the -ish.
#[derive(Debug)]
pub struct Pipeline {}

impl Pipeline {
    pub(crate) fn new(_config: &Config, _tick: Duration) -> Result<Option<Self>, PipelineError> {
        let _sink = PostHogSink::default();
        Ok(None)
    }

    pub fn record(&self, _event: Event) {}
}
