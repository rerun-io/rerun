use std::sync::atomic::AtomicBool;

use re_mutex::Mutex;

use crate::decode::{AsyncDecoder, Chunk, FrameResult, Result};
use crate::{Sender, VideoDataDescription};

pub use super::sync_decoder::SyncDecoder;

/// Runs a [`SyncDecoder`] inline on the caller's thread.
///
/// There is no thread on web, so we just run the decoder inline on `submit_chunk`
/// and `reset`. Callers that submit a large burst of chunks in a single event loop
/// turn will block the browser for the duration of the burst.
pub struct SyncDecoderWrapper {
    /// We own the decoder directly and run it inline.
    ///
    /// Wrapped in a `Mutex` so the wrapper satisfies `AsyncDecoder: Send + Sync`
    /// without an `unsafe impl`. wasm32 is single-threaded, so the lock is never contended.
    sync_decoder: Mutex<Box<dyn SyncDecoder + Send>>,

    output_sender: Sender<FrameResult>,

    /// Passed to [`SyncDecoder::submit_chunk`] so decoders that honor it can early-exit.
    should_stop: AtomicBool,
}

impl SyncDecoderWrapper {
    pub fn new(
        _debug_name: String,
        sync_decoder: Box<dyn SyncDecoder + Send>,
        output_sender: Sender<FrameResult>,
    ) -> Self {
        re_tracing::profile_function!();

        Self {
            sync_decoder: Mutex::new(sync_decoder),
            output_sender,
            should_stop: AtomicBool::new(false),
        }
    }
}

impl AsyncDecoder for SyncDecoderWrapper {
    fn submit_chunk(&mut self, chunk: Chunk) -> Result<()> {
        re_tracing::profile_function!();
        self.sync_decoder
            .lock()
            .submit_chunk(&self.should_stop, chunk, &self.output_sender);
        Ok(())
    }

    /// Resets the decoder synchronously.
    fn reset(&mut self, video_data_description: &VideoDataDescription) -> Result<()> {
        re_tracing::profile_function!();
        self.sync_decoder.lock().reset(video_data_description);
        Ok(())
    }
}
