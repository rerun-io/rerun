use re_chunk::Chunk;
use re_log_types::LogMsg;

use crate::sink::LogSink;

use super::{Lens, ast::LensRegistry};

/// A sink which can transform a [`LogMsg`] and forward the result to an underlying backing [`LogSink`].
///
/// The sink will only forward components that are matched by a lens specified via [`Self::with_lens`].
pub struct LensesSink<S: LogSink> {
    sink: S,
    registry: LensRegistry,
}

impl<S: LogSink> LensesSink<S> {
    /// Creates a new sink without any lenses attached.
    ///
    /// Use [`Self::with_lens`] to add an additional lens to this sink.
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            registry: Default::default(),
        }
    }

    /// Adds a [`Lens`] to this sink.
    pub fn with_lens(mut self, lens: Lens) -> Self {
        self.registry.add_lens(lens);
        self
    }
}

impl<S: LogSink> LogSink for LensesSink<S> {
    fn send(&self, msg: re_log_types::LogMsg) {
        match &msg {
            LogMsg::SetStoreInfo(_) | LogMsg::BlueprintActivationCommand(_) => {
                self.sink.send(msg);
            }
            LogMsg::ArrowMsg(store_id, arrow_msg) => match Chunk::from_arrow_msg(arrow_msg) {
                Ok(chunk) => {
                    let new_chunks = self.registry.apply(&chunk);
                    for new_chunk in new_chunks {
                        match new_chunk.to_arrow_msg() {
                            Ok(arrow_msg) => {
                                self.sink
                                    .send(LogMsg::ArrowMsg(store_id.clone(), arrow_msg));
                            }
                            Err(err) => {
                                re_log::error_once!(
                                    "failed to create log message from chunk: {err}"
                                );
                            }
                        }
                    }
                }

                Err(err) => {
                    re_log::error_once!("Failed to convert arrow message to chunk: {err}");
                    self.sink.send(msg);
                }
            },
        }
    }

    fn flush_blocking(
        &self,
        timeout: std::time::Duration,
    ) -> Result<(), crate::sink::SinkFlushError> {
        self.sink.flush_blocking(timeout)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
