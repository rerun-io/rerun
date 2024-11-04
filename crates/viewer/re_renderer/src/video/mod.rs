mod chunk_decoder;
mod player;

use std::{collections::hash_map::Entry, ops::Range, sync::Arc};

use ahash::HashMap;
use parking_lot::Mutex;

use re_video::{decode::DecodeHardwareAcceleration, VideoData};

use crate::{
    resource_managers::{GpuTexture2D, SourceImageDataFormat},
    RenderContext,
};

/// Error that can occur during playing videos.
#[derive(thiserror::Error, Debug, Clone)]
pub enum VideoPlayerError {
    #[error("The decoder is lagging behind")]
    EmptyBuffer,

    #[error("Video seems to be empty, no segments have beem found.")]
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
    pub texture: GpuTexture2D,

    /// If true, the texture is outdated. Keep polling for a fresh one.
    pub is_pending: bool,

    /// If true, this texture is so out-dated that it should have a loading spinner on top of it.
    pub show_spinner: bool,

    /// Format information about the original data from the video decoder.
    ///
    /// The texture is already converted to something the renderer can use directly.
    pub source_pixel_format: SourceImageDataFormat,

    /// Meta information about the decoded frame.
    pub frame_info: re_video::decode::FrameInfo,
}

impl VideoFrameTexture {
    pub fn time_range(&self) -> Range<re_video::Time> {
        self.frame_info.presentation_timestamp
            ..self.frame_info.presentation_timestamp + self.frame_info.duration
    }
}

/// Identifier for an independent video decoding stream.
///
/// A single video may use several decoders at a time to simultaneously decode frames at different timestamps.
/// The id does not need to be globally unique, just unique enough to distinguish streams of the same video.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]

pub struct VideoPlayerStreamId(pub u64);

struct PlayerEntry {
    player: player::VideoPlayer,
    frame_index: u64,
}

/// Video data + decoder(s).
///
/// Supports asynchronously decoding video into GPU textures via [`Video::frame_at`].
pub struct Video {
    debug_name: String,
    data: Arc<re_video::VideoData>,
    players: Mutex<HashMap<VideoPlayerStreamId, PlayerEntry>>,
    decode_hw_acceleration: DecodeHardwareAcceleration,
}

impl Video {
    /// Loads a video from the given data.
    ///
    /// Currently supports the following media types:
    /// - `video/mp4`
    pub fn load(
        debug_name: String,
        data: Arc<VideoData>,
        decode_hw_acceleration: DecodeHardwareAcceleration,
    ) -> Self {
        let players = Mutex::new(HashMap::default());

        Self {
            debug_name,
            data,
            players,
            decode_hw_acceleration,
        }
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
        player_stream_id: VideoPlayerStreamId,
        presentation_timestamp_s: f64,
        video_data: &[u8],
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
                    render_context,
                    self.data.clone(),
                    self.decode_hw_acceleration,
                )?;
                vacant_entry.insert(PlayerEntry {
                    player: new_player,
                    frame_index: global_frame_idx,
                })
            }
        };

        decoder_entry.frame_index = render_context.active_frame_idx();
        decoder_entry
            .player
            .frame_at(render_context, presentation_timestamp_s, video_data)
    }

    /// Removes all decoders that have been unused in the last frame.
    ///
    /// Decoders are very memory intensive, so they should be cleaned up as soon they're no longer needed.
    pub fn purge_unused_decoders(&self, active_frame_idx: u64) {
        if active_frame_idx == 0 {
            return;
        }

        let mut players = self.players.lock();
        players.retain(|_, decoder| decoder.frame_index >= active_frame_idx - 1);
    }
}
