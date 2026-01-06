use re_chunk::Chunk;
use re_log_types::{LogMsg, StoreId};

use super::Lens;
use super::ast::{Lenses, OutputMode};
use crate::sink::LogSink;

/// A sink which can transform a [`LogMsg`] and forward the result to an underlying backing [`LogSink`].
///
/// The sink will only forward components that are matched by a lens specified via [`Self::with_lens`].
pub struct LensesSink<S: LogSink> {
    sink: S,
    lenses: Lenses,
    strict: bool,
}

impl<S: LogSink> LensesSink<S> {
    /// Creates a new sink without any lenses attached.
    ///
    /// Use [`Self::with_lens`] to add an additional lens to this sink.
    ///
    /// By default, the sink will do its best effort to produce chunks despite
    /// of errors in Lenses that it might encounter.
    pub fn new(sink: S) -> Self {
        Self {
            sink,
            lenses: Lenses::new(OutputMode::DropUnmatched),
            strict: false,
        }
    }

    /// Adds a [`Lens`] to this sink.
    pub fn with_lens(mut self, lens: Lens) -> Self {
        self.lenses.add_lens(lens);
        self
    }

    /// Configure how to handle matched and unmatched data.
    ///
    /// See [`OutputMode`] for more details.
    pub fn output_mode(mut self, mode: OutputMode) -> Self {
        self.lenses.set_output_mode(mode);
        self
    }

    /// When `strict` is `true` Lenses that encounter an error will not emit partial chunks.
    pub fn strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    fn send_or_log_error(&self, store_id: StoreId, chunk: &Chunk) {
        match chunk.to_arrow_msg() {
            Ok(arrow_msg) => {
                self.sink.send(LogMsg::ArrowMsg(store_id, arrow_msg));
            }
            Err(err) => {
                re_log::error_once!("Failed to create log message from chunk: {err}");
            }
        }
    }
}

impl<S: LogSink> LogSink for LensesSink<S> {
    fn send(&self, msg: re_log_types::LogMsg) {
        match &msg {
            LogMsg::SetStoreInfo(_) | LogMsg::BlueprintActivationCommand(_) => {
                self.sink.send(msg);
            }
            LogMsg::ArrowMsg(store_id, arrow_msg) => match Chunk::from_arrow_msg(arrow_msg) {
                Ok(original_chunk) => {
                    let new_chunks = self.lenses.apply(&original_chunk);
                    for maybe_chunk in new_chunks {
                        match maybe_chunk {
                            Ok(new_chunk) => self.send_or_log_error(store_id.clone(), &new_chunk),
                            Err(partial_chunk) => {
                                for error in partial_chunk.errors() {
                                    // TODO(grtlr): Make this even more contextualized in the future!
                                    re_log::error_once!("Error encountered for lens: {error}");
                                }
                                if let Some(chunk) = partial_chunk.take()
                                    && self.strict
                                {
                                    self.send_or_log_error(store_id.clone(), &chunk);
                                }
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
}
