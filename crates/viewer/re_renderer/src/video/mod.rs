mod decoder;

use std::{collections::hash_map::Entry, sync::Arc};

use ahash::HashMap;
use parking_lot::Mutex;

use re_video::VideoLoadError;

use crate::{resource_managers::GpuTexture2D, RenderContext};

/// Error that can occur during frame decoding.
// TODO(jan, andreas): These errors are for the most part specific to the web decoder right now.
#[derive(thiserror::Error, Debug, Clone)]
pub enum DecodingError {
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

    /// e.g. unsupported codec
    #[error("Failed to create video chunk: {0}")]
    CreateChunk(String),

    /// e.g. unsupported codec
    #[error("Failed to decode video chunk: {0}")]
    DecodeChunk(String),

    /// e.g. unsupported codec
    #[error("Failed to decode video: {0}")]
    Decoding(String),

    #[error("The timestamp passed was negative.")]
    NegativeTimestamp,

    /// e.g. bad mp4, or bug in mp4 parse
    #[error("Bad data.")]
    BadData,

    #[cfg(not(target_arch = "wasm32"))]
    #[error("No native video support. Try compiling rerun with the `video_av1` feature flag")]
    NoNativeSupport,

    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(feature = "video_av1")]
    #[error("Unsupported codec: {codec:?}. Only AV1 is currently supported on native.")]
    UnsupportedCodec { codec: String },
}

pub type FrameDecodingResult = Result<VideoFrameTexture, DecodingError>;

/// Information about the status of a frame decoding.
pub enum VideoFrameTexture {
    /// The requested frame got decoded and is ready to be used.
    Ready(GpuTexture2D),

    /// The returned texture is from a previous frame or a placeholder, the decoder is still decoding the requested frame.
    Pending(GpuTexture2D),
}

/// Identifier for an independent video decoding stream.
///
/// A single video may use several decoders at a time to simultaneously decode frames at different timestamps.
/// The id does not need to be globally unique, just unique enough to distinguish streams of the same video.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]

pub struct VideoDecodingStreamId(pub u64);

struct DecoderEntry {
    decoder: Box<dyn decoder::VideoDecoder>,
    frame_index: u64,
}

/// Video data + decoder(s).
///
/// Supports asynchronously decoding video into GPU textures via [`Video::frame_at`].
pub struct Video {
    debug_name: String,
    data: Arc<re_video::VideoData>,
    decoders: Mutex<HashMap<VideoDecodingStreamId, DecoderEntry>>,
    decode_hw_acceleration: DecodeHardwareAcceleration,
}

/// How the video should be decoded.
///
/// Depending on the decoder backend, these settings are merely hints and may be ignored.
/// However, they can be useful in some situations to work around issues.
///
/// On the web this directly corresponds to
/// <https://www.w3.org/TR/webcodecs/#hardware-acceleration>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DecodeHardwareAcceleration {
    /// May use hardware acceleration if available and compatible with the codec.
    #[default]
    Auto,

    /// Should use a software decoder even if hardware acceleration is available.
    ///
    /// If no software decoder is present, this may cause decoding to fail.
    PreferSoftware,

    /// Should use a hardware decoder.
    ///
    /// If no hardware decoder is present, this may cause decoding to fail.
    PreferHardware,
}

impl std::fmt::Display for DecodeHardwareAcceleration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "Auto"),
            Self::PreferSoftware => write!(f, "Prefer software"),
            Self::PreferHardware => write!(f, "Prefer hardware"),
        }
    }
}

impl std::str::FromStr for DecodeHardwareAcceleration {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().replace('-', "_").as_str() {
            "auto" => Ok(Self::Auto),
            "prefer_software" | "software" => Ok(Self::PreferSoftware),
            "prefer_hardware" | "hardware" => Ok(Self::PreferHardware),
            _ => Err(()),
        }
    }
}

impl Video {
    /// Loads a video from the given data.
    ///
    /// Currently supports the following media types:
    /// - `video/mp4`
    pub fn load(
        debug_name: String,
        data: &[u8],
        media_type: Option<&str>,
        decode_hw_acceleration: DecodeHardwareAcceleration,
    ) -> Result<Self, VideoLoadError> {
        let data = Arc::new(re_video::VideoData::load_from_bytes(data, media_type)?);
        let decoders = Mutex::new(HashMap::default());

        Ok(Self {
            debug_name,
            data,
            decoders,
            decode_hw_acceleration,
        })
    }

    /// The video data
    #[inline]
    pub fn data(&self) -> &Arc<re_video::VideoData> {
        &self.data
    }

    /// Natural width of the video.
    #[inline]
    pub fn width(&self) -> u32 {
        self.data.width()
    }

    /// Natural height of the video.
    #[inline]
    pub fn height(&self) -> u32 {
        self.data.height()
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
        presentation_timestamp_s: f64,
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
                let new_decoder = decoder::new_video_decoder(
                    self.debug_name.clone(),
                    render_context,
                    self.data.clone(),
                    self.decode_hw_acceleration,
                )?;
                vacant_entry.insert(DecoderEntry {
                    decoder: new_decoder,
                    frame_index: global_frame_idx,
                })
            }
        };

        decoder_entry.frame_index = render_context.active_frame_idx();
        decoder_entry
            .decoder
            .frame_at(render_context, presentation_timestamp_s)
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
