mod chunk_decoder;
mod player;

use std::collections::hash_map::Entry;

use ahash::HashMap;
use parking_lot::Mutex;

use crate::{
    RenderContext,
    resource_managers::{GpuTexture2D, SourceImageDataFormat},
};
use re_video::{StableIndexDeque, VideoDataDescription, decode::DecodeSettings};

/// Error that can occur during playing videos.
#[derive(thiserror::Error, Debug, Clone)]
pub enum VideoPlayerError {
    #[error("The decoder is lagging behind")]
    EmptyBuffer,

    #[error("Video is empty.")]
    EmptyVideo,

    /// e.g. unsupported codec
    #[error("Failed to create video chunk: {0}")]
    CreateChunk(String),

    /// e.g. unsupported codec
    #[error("Failed to decode video chunk: {0}")]
    DecodeChunk(String),

    /// e.g. unsupported codec
    #[error("Failed to decode video: {0}")]
    Decoding(#[from] re_video::decode::Error),

    #[error("The timestamp passed was negative.")]
    NegativeTimestamp,

    /// e.g. bad mp4, or bug in mp4 parse
    #[error("Bad data.")]
    BadData,

    #[error("Failed to create gpu texture from decoded video data: {0}")]
    ImageDataToTextureError(#[from] crate::resource_managers::ImageDataToTextureError),
}

pub type FrameDecodingResult = Result<VideoFrameTexture, VideoPlayerError>;

/// Information about the status of a frame decoding.
pub struct VideoFrameTexture {
    /// The texture to show.
    pub texture: Option<GpuTexture2D>,

    /// If true, the texture is outdated. Keep polling for a fresh one.
    pub is_pending: bool,

    /// If true, this texture is so out-dated that it should have a loading spinner on top of it.
    pub show_spinner: bool,

    /// Format information about the original data from the video decoder.
    ///
    /// The texture is already converted to something the renderer can use directly.
    pub source_pixel_format: SourceImageDataFormat,

    /// Meta information about the decoded frame.
    pub frame_info: Option<re_video::decode::FrameInfo>,
}

/// Identifier for an independent video decoding stream.
///
/// A single video may use several decoders at a time to simultaneously decode frames at different timestamps.
/// The id does not need to be globally unique, just unique enough to distinguish streams of the same video.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]

pub struct VideoPlayerStreamId(pub u64);

struct PlayerEntry {
    player: player::VideoPlayer,

    /// The global `re_renderer` frame index at which the player was last used.
    /// (this is NOT a video frame index of any kind)
    last_global_frame_idx: u64,
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

    /// Natural dimensions of the video if known.
    #[inline]
    pub fn dimensions(&self) -> Option<[u16; 2]> {
        self.video_description.coded_dimensions
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
    pub fn frame_at(
        &self,
        render_context: &RenderContext,
        player_stream_id: VideoPlayerStreamId,
        time_since_video_start_in_secs: f64,
        video_buffers: &StableIndexDeque<&[u8]>,
    ) -> FrameDecodingResult {
        re_tracing::profile_function!();

        let global_frame_idx = render_context.active_frame_idx();

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
                    last_global_frame_idx: global_frame_idx,
                })
            }
        };

        decoder_entry.last_global_frame_idx = render_context.active_frame_idx();
        decoder_entry.player.frame_at(
            render_context,
            time_since_video_start_in_secs,
            &self.video_description,
            video_buffers,
        )
    }

    /// Removes all decoders that have been unused in the last frame.
    ///
    /// Decoders are very memory intensive, so they should be cleaned up as soon they're no longer needed.
    pub fn purge_unused_decoders(&self, active_frame_idx: u64) {
        if active_frame_idx == 0 {
            return;
        }

        let mut players = self.players.lock();
        players.retain(|_, decoder| decoder.last_global_frame_idx >= active_frame_idx - 1);
    }
}
