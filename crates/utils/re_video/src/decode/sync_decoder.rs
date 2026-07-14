use crate::decode::{Chunk, FrameResult};
use crate::{Sender, VideoDataDescription};

/// Blocking decoder of video chunks.
pub trait SyncDecoder {
    /// Submit some work and read the results.
    ///
    /// Stop early if `should_stop` is `true` or turns `true`.
    fn submit_chunk(
        &mut self,
        should_stop: &std::sync::atomic::AtomicBool,
        chunk: Chunk,
        output_sender: &Sender<FrameResult>,
    );

    /// Clear and reset everything
    fn reset(&mut self, video_data_description: &VideoDataDescription);
}
