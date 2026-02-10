mod chunk_decoder;
mod player;

use std::collections::hash_map::Entry;

use ahash::HashMap;
pub use chunk_decoder::VideoSampleDecoder;
pub use player::{PlayerConfiguration, VideoPlayer};
use re_log::ResultExt as _;
use re_mutex::Mutex;
use re_video::{DecodeSettings, VideoDataDescription};

use crate::RenderContext;
use crate::resource_managers::{GpuTexture2D, SourceImageDataFormat};

/// Detailed error for unloaded samples.
#[derive(thiserror::Error, Debug, Clone)]
pub enum UnloadedSampleDataError {
    #[error("Video doesn't have any loaded samples.")]
    NoLoadedSamples,

    #[error("Frame data required for the requested sample is not loaded yet.")]
    ExpectedSampleNotLoaded,
}

/// Detailed error for lack of sample data.
#[derive(thiserror::Error, Debug, Clone)]
pub enum InsufficientSampleDataError {
    #[error("Video doesn't have any key frames.")]
    NoKeyFrames,

    #[error("Video doesn't have any samples.")]
    NoSamples,

    #[error("No key frames prior to current time.")]
    NoKeyFramesPriorToRequestedTimestamp,

    #[error("No frames prior to current time.")]
    NoSamplesPriorToRequestedTimestamp,

    #[error("Missing samples between last decoded sample and requested sample.")]
    MissingSamples,

    #[error("Duplicate sample index encountered.")]
    DuplicateSampleIdx,

    #[error("Out of order sample index encountered.")]
    OutOfOrderSampleIdx,
}

/// Error that can occur during playing videos.
#[derive(thiserror::Error, Debug, Clone)]
pub enum VideoPlayerError {
    #[error("The decoder is lagging behind")]
    EmptyBuffer,

    #[error(transparent)]
    InsufficientSampleData(#[from] InsufficientSampleDataError),

    #[error(transparent)]
    UnloadedSampleData(#[from] UnloadedSampleDataError),

    /// e.g. unsupported codec
    #[error("Failed to create video chunk: {0}")]
    CreateChunk(String),

    /// e.g. unsupported codec
    #[error("Failed to decode video chunk: {0}")]
    DecodeChunk(String),

    /// Various errors that can occur during video decoding.
    #[error("Failed to decode video: {0}")]
    Decoding(#[from] re_video::DecodeError),

    #[error("The timestamp passed was negative.")]
    NegativeTimestamp,

    /// e.g. bad mp4, or bug in mp4 parse
    #[error("Bad data.")]
    BadData,

    #[error("Failed to create gpu texture from decoded video data: {0}")]
    ImageDataToTextureError(#[from] crate::resource_managers::ImageDataToTextureError),

    #[error("Decoder unexpectedly exited")]
    DecoderUnexpectedlyExited,
}

const _: () = assert!(
    std::mem::size_of::<VideoPlayerError>() <= 64,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

impl VideoPlayerError {
    pub fn should_request_more_frames(&self) -> bool {
        // Decoders often (not always!) recover from errors and will succeed eventually.
        // Gotta keep trying!
        match self {
            Self::Decoding(err) => err.should_request_more_frames(),
            _ => false,
        }
    }
}

pub type FrameDecodingResult = Result<VideoFrameTexture, VideoPlayerError>;

/// Describes whether a decoder is lagging behind or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderDelayState {
    /// The decoder is caught up with the most recent requested frame.
    UpToDate,

    /// We're not up to date, but we're close enough to the newest content of a live stream that we're ok.
    ///
    /// The leading edge of livestreams is treated specially since we don't want to show the waiting indicator
    /// as readily.
    /// Furthermore, it mitigates problems with some decoders not emitting the last few frames until
    /// we signal the end of the video (after which we have to restart the decoder).
    ///
    /// I.e. the video texture may be quite a bit behind, but it's better than not showing new frames.
    /// Unlike with [`DecoderDelayState::UpToDateWithinTolerance`], we won't show a loading indicator.
    ///
    /// The tolerance value used for this is the sum of
    /// [`PlayerConfiguration::tolerated_output_delay_in_num_frames`] and
    /// [`re_video::AsyncDecoder::min_num_samples_to_enqueue_ahead`].
    UpToDateToleratedEdgeOfLiveStream,

    /// The decoder is caught up within a certain tolerance.
    ///
    /// I.e. the video texture is not the most recently requested frame, but it's quite close.
    ///
    /// The tolerance value used for this is [`PlayerConfiguration::tolerated_output_delay_in_num_frames`].
    UpToDateWithinTolerance,

    /// The decoder is catching up after a long seek.
    ///
    /// The video texture is no longer updated until the decoder has caught up.
    /// This state will only be left after reaching [`DecoderDelayState::UpToDate`] again.
    ///
    /// The tolerance value used for this is [`PlayerConfiguration::tolerated_output_delay_in_num_frames`].
    Behind,
}

impl DecoderDelayState {
    /// Whether a user of a video player should keep requesting a more up to date video frame even
    /// if the requested time has not changed.
    pub fn should_request_more_frames(&self) -> bool {
        match self {
            Self::UpToDate => false,

            // Everything that isn't up-to-date means that we have to request more frames
            // since the frame that is displayed right now is the one that was requested.
            Self::UpToDateWithinTolerance
            | Self::Behind
            | Self::UpToDateToleratedEdgeOfLiveStream => true,
        }
    }
}

/// Information about the status of a frame decoding.
pub struct VideoFrameTexture {
    /// The texture to show.
    pub texture: Option<GpuTexture2D>,

    /// If true, the texture is outdated. Keep polling for a fresh one.
    pub decoder_delay_state: DecoderDelayState,

    /// If true, this texture is so out-dated that it should have a loading indicator on top of it.
    pub show_loading_indicator: bool,

    /// Format information about the original data from the video decoder.
    ///
    /// The texture is already converted to something the renderer can use directly.
    pub source_pixel_format: SourceImageDataFormat,

    /// Meta information about the decoded frame.
    pub frame_info: Option<re_video::FrameInfo>,
}

/// Identifier for an independent video decoding stream.
///
/// A single video may use several decoders at a time to simultaneously decode frames at different timestamps.
/// The id does not need to be globally unique, just unique enough to distinguish streams of the same video.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]

pub struct VideoPlayerStreamId(pub u64);

impl re_byte_size::SizeBytes for VideoPlayerStreamId {
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    fn is_pod() -> bool {
        true
    }
}

struct PlayerEntry {
    player: player::VideoPlayer,

    /// Was this used last frame?
    /// This is reset every frame, and used to determine whether to purge the player.
    used_last_frame: bool,
}

impl re_byte_size::SizeBytes for PlayerEntry {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            player,
            used_last_frame: _,
        } = self;
        player.heap_size_bytes()
    }
}

/// Video data + decoder(s).
///
/// Supports asynchronously decoding video into GPU textures via [`Video::frame_at`].
pub struct Video {
    debug_name: String,
    video_description: re_video::VideoDataDescription,
    players: Mutex<HashMap<VideoPlayerStreamId, PlayerEntry>>,
    decode_settings: DecodeSettings,
}

impl re_byte_size::SizeBytes for Video {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            debug_name,
            video_description,
            players,
            decode_settings: _,
        } = self;
        debug_name.heap_size_bytes()
            + video_description.heap_size_bytes()
            + players.lock().heap_size_bytes()
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        re_log::trace!("Dropping Video {:?}", self.debug_name);
    }
}

impl Video {
    /// Loads a video from the given data.
    ///
    /// Currently supports the following media types:
    /// - `video/mp4`
    pub fn load(
        debug_name: String,
        video_description: VideoDataDescription,
        decode_settings: DecodeSettings,
    ) -> Self {
        let players = Mutex::new(HashMap::default());

        Self {
            debug_name,
            video_description,
            players,
            decode_settings,
        }
    }

    /// The video description.
    #[inline]
    pub fn data_descr(&self) -> &re_video::VideoDataDescription {
        &self.video_description
    }

    /// Mutable access to the video data.
    ///
    /// Use with care. It's valid to add samples and groups of pictures, but arbitrary
    /// changes may interfere with subsequent decoding on existing video streams.
    #[inline]
    pub fn data_descr_mut(&mut self) -> &mut re_video::VideoDataDescription {
        &mut self.video_description
    }

    /// Resets all decoders and purges any cached frames.
    ///
    /// This is useful when the video description has changed since the decoders were created.
    pub fn reset_all_decoders(&self) {
        let mut players = self.players.lock();
        for player in players.values_mut() {
            player
                .player
                .reset(&self.video_description)
                .ok_or_log_error_once();
        }
    }

    /// Natural dimensions of the video if known.
    #[inline]
    pub fn dimensions(&self) -> Option<[u16; 2]> {
        self.video_description
            .encoding_details
            .as_ref()
            .map(|details| details.coded_dimensions)
    }

    /// Returns a texture with the latest frame at the given time since video start.
    ///
    /// If the time is negative, a zeroed texture is returned.
    ///
    /// This API is _asynchronous_, meaning that the decoder may not yet have decoded the frame
    /// at the given timestamp. If the frame is not yet available, the returned texture will be
    /// empty.
    ///
    /// The time is specified in seconds since the start of the video.
    ///
    /// `get_video_buffer` is used both to read data for frames internally, and as a way to request
    /// what data should be loaded.
    pub fn frame_at<'a>(
        &self,
        render_context: &RenderContext,
        player_stream_id: VideoPlayerStreamId,
        video_time: re_video::Time,
        get_video_buffer: &dyn Fn(re_tuid::Tuid) -> &'a [u8],
    ) -> FrameDecodingResult {
        re_tracing::profile_function!();

        // We could protect this hashmap by a RwLock and the individual decoders by a Mutex.
        // However, dealing with the RwLock efficiently is complicated:
        // Upgradable-reads exclude other upgradable-reads which means that if an element is not found,
        // we have to drop the unlock and relock with a write lock, during which new elements may be inserted.
        // This can be overcome by looping until successful, or instead we can just use a single Mutex lock and leave it there.
        let mut players = self.players.lock();
        let decoder_entry = match players.entry(player_stream_id) {
            Entry::Occupied(occupied_entry) => occupied_entry.into_mut(),
            Entry::Vacant(vacant_entry) => {
                let new_player = player::VideoPlayer::new(
                    &self.debug_name,
                    &self.video_description,
                    &self.decode_settings,
                )?;
                vacant_entry.insert(PlayerEntry {
                    player: new_player,
                    used_last_frame: true,
                })
            }
        };

        decoder_entry.used_last_frame = true;
        decoder_entry.player.frame_at(
            video_time,
            &self.video_description,
            &mut |texture, frame| {
                chunk_decoder::update_video_texture_with_frame(render_context, texture, frame)
            },
            get_video_buffer,
        )
    }

    /// Removes all decoders that have been unused in the last frame.
    ///
    /// Decoders are very memory intensive, so they should be cleaned up as soon they're no longer needed.
    pub fn begin_frame(&self) {
        re_tracing::profile_function!();

        let mut players = self.players.lock();
        players.retain(|_id, entry| entry.used_last_frame);

        // Reset for the next frame:
        for entry in players.values_mut() {
            entry.used_last_frame = false;
        }
    }
}
