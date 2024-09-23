mod decoder;

use ahash::HashMap;
use parking_lot::Mutex;
use std::{collections::hash_map::Entry, sync::Arc};

use re_video::VideoLoadError;

use crate::{resource_managers::GpuTexture2D, RenderContext};

/// Error that can occur during frame decoding.
// TODO(jan, andreas): These errors are for the most part specific to the web decoder right now.
#[derive(thiserror::Error, Debug)]
pub enum DecodingError {
    // TODO(#7298): Native support.
    #[error("Video playback not yet available in the native viewer. Try the web viewer instead.")]
    NoNativeSupport,

    #[error("Failed to create VideoDecoder: {0}")]
    DecoderSetupFailure(String),

    #[error("Video seems to be empty, no segments have beem found.")]
    EmptyVideo,

    #[error("The current segment is empty.")]
    EmptySegment,

    #[error("Failed to reset the decoder: {0}")]
    ResetFailure(String),

    #[error("Failed to configure the video decoder: {0}")]
    ConfigureFailure(String),

    #[error("The timestamp passed was negative.")]
    NegativeTimestamp,
}

/// Information about the status of a frame decoding.
pub enum FrameDecodingResult {
    /// The requested frame got decoded and is ready to be used.
    Ready(GpuTexture2D),

    /// The returned texture is from a previous frame or a placeholder, the decoder is still decoding the requested frame.
    Pending(GpuTexture2D),

    /// The decoder encountered an error and was not able to produce a texture for the requested timestamp.
    /// The returned texture is either a placeholder or the last successfully decoded texture.
    Error(DecodingError),
}

/// Identifier for an independent video decoding stream.
///
/// A single video may use several decoders at a time to simultaneously decode frames at different timestamps.
/// The id does not need to be globally unique, just unique enough to distinguish streams of the same video.
pub type VideoDecodingStreamId = u64;

struct DecoderEntry {
    decoder: decoder::VideoDecoder,
    frame_index: u64,
}

/// Video data + decoder(s).
///
/// Supports asynchronously decoding video into GPU textures via [`Video::frame_at`].
pub struct Video {
    data: Arc<re_video::VideoData>,
    decoders: Mutex<HashMap<VideoDecodingStreamId, DecoderEntry>>,
}

impl Video {
    /// Loads a video from the given data.
    ///
    /// Currently supports the following media types:
    /// - `video/mp4`
    pub fn load(data: &[u8], media_type: Option<&str>) -> Result<Self, VideoLoadError> {
        let data = Arc::new(re_video::VideoData::load_from_bytes(data, media_type)?);
        let decoders = Mutex::new(HashMap::default());

        Ok(Self { data, decoders })
    }

    /// Duration of the video, in milliseconds.
    pub fn duration_ms(&self) -> f64 {
        self.data.duration.into_millis(self.data.timescale)
    }

    /// Natural width of the video.
    pub fn width(&self) -> u32 {
        self.data.config.coded_width as u32
    }

    /// Natural height of the video.
    pub fn height(&self) -> u32 {
        self.data.config.coded_height as u32
    }

    /// The codec used to encode the video.
    pub fn codec(&self) -> &str {
        &self.data.config.codec
    }

    /// The number of samples in the video.
    pub fn num_samples(&self) -> usize {
        self.data.samples.len()
    }

    /// Returns a texture with the latest frame at the given timestamp.
    ///
    /// If the timestamp is negative, a zeroed texture is returned.
    ///
    /// This API is _asynchronous_, meaning that the decoder may not yet have decoded the frame
    /// at the given timestamp. If the frame is not yet available, the returned texture will be
    /// empty.
    pub fn frame_at(
        &self,
        render_context: &RenderContext,
        decoder_stream_id: VideoDecodingStreamId,
        timestamp_s: f64,
    ) -> FrameDecodingResult {
        re_tracing::profile_function!();

        let global_frame_idx = render_context.active_frame_idx();

        // We could protect this hashmap by a RwLock and the individual decoders by a Mutex.
        // However, dealing with the RwLock efficiently is complicated:
        // Upgradable-reads exclude other upgradable-reads which means that if an element is not found,
        // we have to drop the unlock and relock with a write lock, during which new elements may be inserted.
        // This can be overcome by looping until successful, or instead we can just use a single Mutex lock and leave it there.
        let mut decoders = self.decoders.lock();
        let decoder_entry = match decoders.entry(decoder_stream_id) {
            Entry::Occupied(occupied_entry) => occupied_entry.into_mut(),
            Entry::Vacant(vacant_entry) => {
                let new_decoder =
                    match decoder::VideoDecoder::new(render_context, self.data.clone()) {
                        Ok(decoder) => decoder,
                        Err(err) => {
                            return FrameDecodingResult::Error(err);
                        }
                    };
                vacant_entry.insert(DecoderEntry {
                    decoder: new_decoder,
                    frame_index: global_frame_idx,
                })
            }
        };

        decoder_entry.frame_index = render_context.active_frame_idx();
        decoder_entry.decoder.frame_at(render_context, timestamp_s)
    }

    /// Removes all decoders that have been unused in the last frame.
    ///
    /// Decoders are very memory intensive, so they should be cleaned up as soon they're no longer needed.
    pub fn purge_unused_decoders(&self, active_frame_idx: u64) {
        if active_frame_idx == 0 {
            return;
        }

        let mut decoders = self.decoders.lock();
        decoders.retain(|_, decoder| decoder.frame_index >= active_frame_idx - 1);
    }
}
