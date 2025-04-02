use std::{sync::Arc, time::Duration};

use web_time::Instant;

use re_video::{
    decode::{DecodeSettings, FrameInfo},
    Time,
};

use super::{chunk_decoder::VideoChunkDecoder, VideoFrameTexture};
use crate::{
    resource_managers::{GpuTexture2D, SourceImageDataFormat},
    video::VideoPlayerError,
    wgpu_resources::{GpuTexturePool, TextureDesc},
    RenderContext,
};

/// Ignore hickups lasting shorter than this.
///
/// Delaying error reports (and showing last-good images meanwhile) allows us to skip over
/// transient errors without flickering.
///
/// Same with showing a spinner: if we show it too fast, it is annoying.
const DECODING_GRACE_DELAY: Duration = Duration::from_millis(400);

#[derive(Debug)]
pub struct TimedDecodingError {
    time_of_first_error: Instant,
    pub latest_error: VideoPlayerError,
}

impl TimedDecodingError {
    pub fn new(latest_error: VideoPlayerError) -> Self {
        Self {
            time_of_first_error: Instant::now(),
            latest_error,
        }
    }
}

/// A texture of a specific video frame.
pub struct VideoTexture {
    pub texture: GpuTexture2D,
    pub frame_info: Option<FrameInfo>,
    pub source_pixel_format: SourceImageDataFormat,
}

/// Decode video to a texture, optimized for extracting successive frames over time.
///
/// If you want to sample multiple points in a video simultaneously, use multiple video players.
pub struct VideoPlayer {
    data: Arc<re_video::VideoData>,
    chunk_decoder: VideoChunkDecoder,

    video_texture: VideoTexture,

    last_requested_sample_idx: usize,
    last_requested_gop_idx: usize,
    last_enqueued_gop_idx: Option<usize>,

    /// Last error that was encountered during decoding.
    ///
    /// Only fully reset after a successful decode.
    last_error: Option<TimedDecodingError>,
}

impl VideoPlayer {
    pub fn new(
        debug_name: &str,
        render_ctx: &RenderContext,
        data: Arc<re_video::VideoData>,
        decode_settings: &DecodeSettings,
    ) -> Result<Self, VideoPlayerError> {
        let debug_name = format!(
            "{debug_name}, codec: {}",
            data.human_readable_codec_string()
        );

        if let Some(bit_depth) = data.config.stsd.contents.bit_depth() {
            #[allow(clippy::comparison_chain)]
            if bit_depth < 8 {
                re_log::warn_once!("{debug_name} has unusual bit_depth of {bit_depth}");
            } else if 8 < bit_depth {
                re_log::warn_once!(
                    "{debug_name}: HDR videos not supported. See https://github.com/rerun-io/rerun/issues/7594 for more."
                );
            }
        }

        let chunk_decoder = VideoChunkDecoder::new(debug_name.clone(), |on_output| {
            re_video::decode::new_decoder(&debug_name, &data, decode_settings, on_output)
        })?;

        let texture = alloc_video_frame_texture(
            &render_ctx.device,
            &render_ctx.gpu_resources.textures,
            data.config.coded_width as u32,
            data.config.coded_height as u32,
        );

        Ok(Self {
            data,
            chunk_decoder,

            video_texture: VideoTexture {
                texture,
                frame_info: None,
                source_pixel_format: SourceImageDataFormat::WgpuCompatible(
                    wgpu::TextureFormat::Rgba8Unorm,
                ),
            },

            last_requested_sample_idx: usize::MAX,
            last_requested_gop_idx: usize::MAX,
            last_enqueued_gop_idx: None,

            last_error: None,
        })
    }

    /// Get the video frame at the given time stamp.
    ///
    /// This will seek in the video if needed.
    /// If you want to sample multiple points in a video simultaneously, use multiple decoders.
    pub fn frame_at(
        &mut self,
        render_ctx: &RenderContext,
        time_since_video_start_in_secs: f64,
        video_data: &[u8],
    ) -> Result<VideoFrameTexture, VideoPlayerError> {
        if time_since_video_start_in_secs < 0.0 {
            return Err(VideoPlayerError::NegativeTimestamp);
        }
        let presentation_timestamp =
            Time::from_secs(time_since_video_start_in_secs, self.data.timescale);
        let presentation_timestamp = presentation_timestamp.min(self.data.duration); // Don't seek past the end of the video.

        let error_on_last_frame_at = self.last_error.is_some();
        self.enqueue_samples(presentation_timestamp, video_data)?;
        self.update_video_texture(render_ctx, presentation_timestamp)?;

        let frame_info = self.video_texture.frame_info.clone();

        if let Some(frame_info) = frame_info {
            let time_range = frame_info.presentation_time_range();
            let is_active_frame = time_range.contains(&presentation_timestamp);

            let is_pending = !is_active_frame;

            let show_spinner = if is_pending && error_on_last_frame_at {
                // If we switched from error to pending, clear the texture.
                // This is important to avoid flickering, in particular when switching from
                // benign errors like DecodingError::NegativeTimestamp.
                // If we don't do this, we see the last valid texture which can look really weird.
                clear_texture(render_ctx, &self.video_texture.texture);
                self.video_texture.frame_info = None;
                true
            } else if presentation_timestamp < time_range.start {
                // We're seeking backwards and somehow forgot to reset.
                true
            } else if presentation_timestamp < time_range.end {
                false // it is an active frame
            } else {
                let how_outdated = presentation_timestamp - time_range.end;
                if how_outdated.duration(self.data.timescale) < DECODING_GRACE_DELAY {
                    false // Just outdated by a little bit - show no spinner
                } else {
                    true // Very old frame - show spinner
                }
            };
            Ok(VideoFrameTexture {
                texture: self.video_texture.texture.clone(),
                is_pending,
                show_spinner,
                frame_info: Some(frame_info),
                source_pixel_format: self.video_texture.source_pixel_format,
            })
        } else {
            Ok(VideoFrameTexture {
                texture: self.video_texture.texture.clone(),
                is_pending: true,
                show_spinner: true,
                frame_info: None,
                source_pixel_format: self.video_texture.source_pixel_format,
            })
        }
    }

    fn enqueue_samples(
        &mut self,
        presentation_timestamp: Time,
        video_data: &[u8],
    ) -> Result<(), VideoPlayerError> {
        re_tracing::profile_function!();

        // Some terminology:
        //   - presentation timestamp (PTS) == composition timestamp
        //     = the time at which the frame should be shown
        //   - decode timestamp (DTS)
        //     = determines the decoding order of samples
        //
        // Note: `decode <= composition` for any given sample.
        //       For some codecs & videos, the two timestamps are the same.
        // We must enqueue samples in decode order, but show them in composition order.
        // In the presence of b-frames this order may be different!

        // Find sample which when decoded will be presented at the timestamp the user requested.
        let requested_sample_idx = self
            .data
            .latest_sample_index_at_presentation_timestamp(presentation_timestamp)
            .ok_or(VideoPlayerError::EmptyVideo)?;

        // Find the GOP that contains the sample.
        let requested_gop_idx = self
            .data
            .gop_index_containing_decode_timestamp(
                self.data.samples[requested_sample_idx].decode_timestamp,
            )
            .ok_or(VideoPlayerError::EmptyVideo)?;

        // Enqueue GOPs as needed.

        // First, check for decoding errors that may have been set asynchronously and reset.
        if let Some(error) = self.chunk_decoder.take_error() {
            // For each new (!) error after entering the error state, we reset the decoder.
            // This way, it might later recover from the error as we progress in the video.
            //
            // By resetting the current GOP/sample indices, the frame enqueued code below
            // is forced to reset the decoder.
            self.last_requested_gop_idx = usize::MAX;

            // If we already have an error set, preserve its occurrence time.
            // Otherwise, set the error using the time at which it was registered.
            if let Some(last_error) = &mut self.last_error {
                last_error.latest_error = error.latest_error;
            } else {
                self.last_error = Some(error);
            }
        }

        // Check all cases in which we have to reset the decoder.
        // This is everything that goes backwards or jumps a GOP.
        if requested_gop_idx != self.last_requested_gop_idx {
            // Backward seeks or seeks across many GOPs trigger a reset of the decoder,
            // because decoding all the samples between the previous sample and the requested
            // one would mean decoding and immediately discarding more frames than we need.
            if self.last_requested_gop_idx.saturating_add(1) != requested_gop_idx {
                self.reset()?;
            }
        } else if requested_sample_idx != self.last_requested_sample_idx {
            let current_pts =
                self.data.samples[self.last_requested_sample_idx].presentation_timestamp;
            let requested_sample = &self.data.samples[requested_sample_idx];

            if requested_sample.presentation_timestamp < current_pts {
                re_log::trace!(
                    "Seeking backwards to sample {requested_sample_idx} (frame_nr {})",
                    requested_sample.frame_nr
                );

                // special case: handle seeking backwards within a single GOP
                // this is super inefficient, but it's the only way to handle it
                // while maintaining a buffer of only 2 GOPs
                //
                // Note that due to sample reordering (in the presence of b-frames), if can happen
                // that `self.current_sample_idx` is *behind* the `requested_sample_idx` even if we're
                // seeking backwards!
                // Therefore, it's important to compare presentation timestamps instead of sample indices.
                // (comparing decode timestamps should be equivalent to comparing sample indices)
                self.reset()?;
            }
        }

        // Ensure that we have as many GOPs enqueued currently as needed in order to…
        // * cover the GOP of the requested sample _plus one_ so we can always smoothly transition to the next GOP
        // * cover at least `min_num_samples_to_enqueue_ahead` samples to work around issues with some decoders
        //   (note that for large GOPs this is usually irrelevant)
        //
        // (potentially related to:) TODO(#7327, #7595): We don't necessarily have to enqueue full GOPs always.
        // In particularly beyond `requested_gop_idx` this can be overkill.
        let min_end_sample_idx =
            requested_sample_idx + self.chunk_decoder.min_num_samples_to_enqueue_ahead();
        loop {
            let next_gop_idx = if let Some(last_enqueued_gop_idx) = self.last_enqueued_gop_idx {
                let last_enqueued_gop = self.data.gops.get(last_enqueued_gop_idx);
                let last_enqueued_sample_idx = last_enqueued_gop
                    .map(|gop| gop.sample_range_usize().end)
                    .unwrap_or(0);

                if last_enqueued_gop_idx > requested_gop_idx // Enqueue the next GOP after requested as well.
                    && last_enqueued_sample_idx >= min_end_sample_idx
                {
                    break;
                }
                last_enqueued_gop_idx + 1
            } else {
                requested_gop_idx
            };

            if next_gop_idx >= self.data.gops.len() {
                // Reached end of video with a previously enqueued GOP already.
                break;
            }

            self.enqueue_gop(next_gop_idx, video_data)?;
        }

        self.last_requested_sample_idx = requested_sample_idx;
        self.last_requested_gop_idx = requested_gop_idx;

        Ok(())
    }

    fn update_video_texture(
        &mut self,
        render_ctx: &RenderContext,
        presentation_timestamp: Time,
    ) -> Result<(), VideoPlayerError> {
        let result = self.chunk_decoder.update_video_texture(
            render_ctx,
            &mut self.video_texture,
            presentation_timestamp,
        );

        if let Err(err) = result {
            if matches!(err, VideoPlayerError::EmptyBuffer) {
                // No buffered frames

                // Might this be due to an error?
                //
                // We only care about decoding errors when we don't find the requested frame,
                // since we want to keep playing the video fine even if parts of it are broken.
                // That said, practically we reset the decoder and thus all frames upon error,
                // so it doesn't make a lot of difference.
                if let Some(timed_error) = &self.last_error {
                    if DECODING_GRACE_DELAY <= timed_error.time_of_first_error.elapsed() {
                        // Report the error only if we have been in an error state for a certain amount of time.
                        // Don't immediately report the error, since we might immediately recover from it.
                        // Otherwise, this would cause aggressive flickering!
                        return Err(timed_error.latest_error.clone());
                    }
                }

                // Don't return a zeroed texture, because we may just be behind on decoding
                // and showing an old frame is better than showing a blank frame,
                // because it causes "black flashes" to appear
                Ok(())
            } else {
                Err(err)
            }
        } else {
            self.last_error = None;
            Ok(())
        }
    }

    /// Enqueue all samples in the given GOP.
    ///
    /// Does nothing if the index is out of bounds.
    fn enqueue_gop(&mut self, gop_idx: usize, video_data: &[u8]) -> Result<(), VideoPlayerError> {
        let Some(gop) = self.data.gops.get(gop_idx) else {
            return Ok(());
        };

        self.last_enqueued_gop_idx = Some(gop_idx);

        let samples = &self.data.samples[gop.sample_range_usize()];

        re_log::trace!("Enqueueing GOP {gop_idx} ({} samples)", samples.len());

        for sample in samples {
            let chunk = sample.get(video_data).ok_or(VideoPlayerError::BadData)?;
            self.chunk_decoder.decode(chunk)?;
        }

        if gop_idx + 1 == self.data.gops.len() {
            // Last GOP - there is nothing more to decode,
            // so flush out any pending frames:
            // See https://github.com/rerun-io/rerun/issues/8073
            self.chunk_decoder.end_of_video()?;
        }

        Ok(())
    }

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self) -> Result<(), VideoPlayerError> {
        self.chunk_decoder.reset()?;
        self.last_requested_gop_idx = usize::MAX;
        self.last_requested_sample_idx = usize::MAX;
        self.last_enqueued_gop_idx = None;
        // Do *not* reset the error state. We want to keep track of the last error.
        Ok(())
    }
}

#[allow(unused)] // For some feature flags
fn alloc_video_frame_texture(
    device: &wgpu::Device,
    pool: &GpuTexturePool,
    width: u32,
    height: u32,
) -> GpuTexture2D {
    let Some(texture) = GpuTexture2D::new(pool.alloc(
        device,
        &TextureDesc {
            label: "video".into(),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            // Needs [`wgpu::TextureUsages::RENDER_ATTACHMENT`], otherwise copy of external textures will fail.
            // Adding [`wgpu::TextureUsages::COPY_SRC`] so we can read back pixels on demand.
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
        },
    )) else {
        // We set the dimension to `2D` above, so this should never happen.
        unreachable!();
    };

    texture
}

/// Clears the texture that is shown on pending to black.
fn clear_texture(render_ctx: &RenderContext, texture: &GpuTexture2D) {
    // Clear texture is a native only feature, so let's not do that.
    // before_view_builder_encoder.clear_texture(texture, subresource_range);

    // But our target is also a render target, so just create a dummy renderpass with clear.
    let mut before_view_builder_encoder =
        render_ctx.active_frame.before_view_builder_encoder.lock();
    let _ = before_view_builder_encoder
        .get()
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            label: crate::DebugLabel::from("clear_video_texture").get(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture.default_view,
                resolve_target: None,
                ops: wgpu::Operations::<wgpu::Color> {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
}
