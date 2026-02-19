mod chunk_decoder;

use std::collections::hash_map::Entry;

use ahash::HashMap;
use re_log::ResultExt as _;
use re_mutex::Mutex;
use re_video::player::{DecoderDelayState, VideoPlayerError, VideoPlayerStreamId};
use re_video::{DecodeSettings, VideoDataDescription};

use crate::RenderContext;
use crate::resource_managers::{GpuTexture2D, SourceImageDataFormat};

/// A [`re_video::player::VideoPlayer`] with GPU texture output.
pub type VideoPlayer = re_video::player::VideoPlayer<VideoTexture>;

impl From<crate::resource_managers::ImageDataToTextureError> for VideoPlayerError {
    fn from(err: crate::resource_managers::ImageDataToTextureError) -> Self {
        Self::TextureUploadError(err.to_string())
    }
}

pub type FrameDecodingResult = Result<VideoFrameTexture, VideoPlayerError>;

/// A texture of a specific video frame.
#[derive(Clone)]
pub struct VideoTexture {
    /// The video texture is created lazily on the first received frame.
    pub texture: Option<GpuTexture2D>,
    pub source_pixel_format: SourceImageDataFormat,
}

impl Default for VideoTexture {
    fn default() -> Self {
        Self {
            texture: None,
            source_pixel_format: SourceImageDataFormat::WgpuCompatible(
                wgpu::TextureFormat::Rgba8Unorm,
            ),
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

struct PlayerEntry {
    player: VideoPlayer,

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

    pub fn debug_name(&self) -> &str {
        &self.debug_name
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
                let new_player = VideoPlayer::new(
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
        let status = decoder_entry.player.frame_at(
            video_time,
            &self.video_description,
            &mut |texture, frame| {
                chunk_decoder::update_video_texture_with_frame(render_context, texture, frame)
            },
            get_video_buffer,
        )?;

        let output = decoder_entry.player.output();
        Ok(VideoFrameTexture {
            texture: output.and_then(|o| o.texture.clone()),
            decoder_delay_state: status.decoder_delay_state,
            show_loading_indicator: status.show_loading_indicator,
            source_pixel_format: output.map_or(
                SourceImageDataFormat::WgpuCompatible(wgpu::TextureFormat::Rgba8Unorm),
                |o| o.source_pixel_format,
            ),
            frame_info: status.frame_info,
        })
    }

    /// Notify players that samples were inserted/removed at `splice_start`,
    /// changing the deque size by `size_delta`.
    ///
    /// Players whose active decoder pipeline overlaps the insertion region
    /// are fully reset. All others just have their indices shifted.
    pub fn handle_sample_insertion(
        &self,
        change_start: re_video::SampleIndex,
        change_end: re_video::SampleIndex,
        size_delta: isize,
    ) {
        let mut players = self.players.lock();
        for entry in players.values_mut() {
            let player = &mut entry.player;

            // If the insertion falls within the player's active GOP range
            // the decoder has a hole of missing samples and needs reset.
            let needs_reset = {
                let indices = player
                    .last_enqueued()
                    .into_iter()
                    .chain(player.last_requested());

                indices
                    .map(|idx| {
                        let keyframe_idx = self.video_description.sample_keyframe_idx(idx)?;

                        self.video_description
                            .gop_sample_range_for_keyframe(keyframe_idx)
                    })
                    .reduce(|a, b| {
                        a.zip(b)
                            .map(|(a, b)| a.start.min(b.start)..a.end.max(b.end))
                    })
                    .flatten()
                    .is_some_and(|gop| {
                        // Check if the gop range overlaps with the changed samples range.
                        gop.start <= change_end && change_start < gop.end
                    })
            };

            if needs_reset {
                player.reset(&self.video_description).ok_or_log_error_once();
            } else {
                // Shift player indices to the new index space.
                player.shift_indices(change_end, size_delta);
            }
        }
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
