use std::time::Duration;

use web_time::Instant;

use re_video::{DecodeSettings, FrameInfo, GopIndex, SampleIndex, StableIndexDeque, Time};

use super::{VideoFrameTexture, chunk_decoder::VideoSampleDecoder};
use crate::{
    RenderContext,
    resource_managers::{GpuTexture2D, SourceImageDataFormat},
    video::VideoPlayerError,
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
#[derive(Clone)]
pub struct VideoTexture {
    /// The video texture is created lazily on the first received frame.
    pub texture: Option<GpuTexture2D>,
    pub frame_info: Option<FrameInfo>,
    pub source_pixel_format: SourceImageDataFormat,
}

/// Decode video to a texture, optimized for extracting successive frames over time.
///
/// If you want to sample multiple points in a video simultaneously, use multiple video players.
pub struct VideoPlayer {
    chunk_decoder: VideoSampleDecoder,

    video_texture: VideoTexture,

    last_requested_sample_idx: SampleIndex,
    last_requested_gop_idx: GopIndex,
    last_enqueued_gop_idx: Option<GopIndex>,

    /// Last error that was encountered during decoding.
    ///
    /// Only fully reset after a successful decode.
    last_error: Option<TimedDecodingError>,
}

impl VideoPlayer {
    /// Create a new video player for a given video.
    ///
    /// The video data description may change over time by adding and removing samples and GOPs,
    /// but other properties are expected to be stable.
    pub fn new(
        debug_name: &str,
        description: &re_video::VideoDataDescription,
        decode_settings: &DecodeSettings,
    ) -> Result<Self, VideoPlayerError> {
        let debug_name = format!(
            "{debug_name}, codec: {}",
            description.human_readable_codec_string()
        );

        if let Some(details) = description.encoding_details.as_ref() {
            if let Some(bit_depth) = details.bit_depth {
                #[allow(clippy::comparison_chain)]
                if bit_depth < 8 {
                    re_log::warn_once!("{debug_name} has unusual bit_depth of {bit_depth}");
                } else if 8 < bit_depth {
                    re_log::warn_once!(
                        "{debug_name}: HDR videos not supported. See https://github.com/rerun-io/rerun/issues/7594 for more."
                    );
                }
            }
        }

        let chunk_decoder = VideoSampleDecoder::new(debug_name.clone(), |on_output| {
            re_video::new_decoder(&debug_name, description, decode_settings, on_output)
        })?;

        Ok(Self {
            chunk_decoder,

            video_texture: VideoTexture {
                texture: None,
                frame_info: None,
                source_pixel_format: SourceImageDataFormat::WgpuCompatible(
                    wgpu::TextureFormat::Rgba8Unorm,
                ),
            },

            last_requested_sample_idx: SampleIndex::MAX,
            last_requested_gop_idx: GopIndex::MAX,
            last_enqueued_gop_idx: None,

            last_error: None,
        })
    }

    /// Get the video frame at the given time stamp.
    ///
    /// This will seek in the video if needed.
    /// If you want to sample multiple points in a video simultaneously, use multiple decoders.
    ///
    /// The video data description may change over time by adding and removing samples and GOPs,
    /// but other properties are expected to be stable.
    pub fn frame_at(
        &mut self,
        render_ctx: &RenderContext,
        video_time: Time,
        video_description: &re_video::VideoDataDescription,
        video_buffers: &StableIndexDeque<&[u8]>,
    ) -> Result<VideoFrameTexture, VideoPlayerError> {
        if video_time.0 < 0 {
            return Err(VideoPlayerError::NegativeTimestamp);
        }
        let mut presentation_timestamp = video_time;
        if let Some(duration) = video_description.duration {
            presentation_timestamp = presentation_timestamp.min(duration); // Don't seek past the end of the video.
        }

        let error_on_last_frame_at = self.last_error.is_some();
        self.enqueue_samples(video_description, presentation_timestamp, video_buffers)?;

        update_video_texture(
            render_ctx,
            &self.chunk_decoder,
            &mut self.last_error,
            &mut self.video_texture,
            presentation_timestamp,
        )?;

        let (is_pending, show_spinner) = if let (Some(frame_info), Some(video_gpu_texture)) = (
            self.video_texture.frame_info.clone(),
            self.video_texture.texture.clone(),
        ) {
            let time_range = frame_info.presentation_time_range();
            let is_active_frame = time_range.contains(&presentation_timestamp);

            let is_pending = !is_active_frame;
            let show_spinner = if is_pending && error_on_last_frame_at {
                // If we switched from error to pending, clear the texture.
                // This is important to avoid flickering, in particular when switching from
                // benign errors like DecodingError::NegativeTimestamp.
                // If we don't do this, we see the last valid texture which can look really weird.
                clear_texture(render_ctx, &video_gpu_texture);
                self.video_texture.frame_info = None;
                true
            } else if presentation_timestamp < time_range.start {
                // We're seeking backwards and somehow forgot to reset.
                true
            } else if presentation_timestamp < time_range.end {
                false // it is an active frame
            } else if let Some(timescale) = video_description.timescale {
                let how_outdated = presentation_timestamp - time_range.end;
                if how_outdated.duration(timescale) < DECODING_GRACE_DELAY {
                    false // Just outdated by a little bit - show no spinner
                } else {
                    true // Very old frame - show spinner
                }
            } else {
                // TODO(andreas): Too much spinner? configure this from the outside in video time units!
                true // No timescale - show spinner too often rather than too late
            };
            (is_pending, show_spinner)
        } else {
            (true, true)
        };

        Ok(VideoFrameTexture {
            texture: self.video_texture.texture.clone(),
            is_pending,
            show_spinner,
            frame_info: self.video_texture.frame_info.clone(),
            source_pixel_format: self.video_texture.source_pixel_format,
        })
    }

    // TODO(#7484): Need to revisit this for gops that expand over time due to newly arrived samples.
    fn enqueue_samples(
        &mut self,
        video_description: &re_video::VideoDataDescription,
        presentation_timestamp: Time,
        video_buffers: &StableIndexDeque<&[u8]>,
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
        let requested_sample_idx = video_description
            .latest_sample_index_at_presentation_timestamp(presentation_timestamp)
            .ok_or(VideoPlayerError::EmptyVideo)?;

        // Find the GOP that contains the sample.
        let requested_gop_idx = video_description
            .gop_index_containing_decode_timestamp(
                video_description.samples[requested_sample_idx].decode_timestamp,
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
            self.last_requested_gop_idx = GopIndex::MAX;

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
                self.reset(video_description)?;
            }
        } else if requested_sample_idx != self.last_requested_sample_idx {
            let requested_sample = video_description
                .samples
                .get(self.last_requested_sample_idx); // If it is not available, it got GC'ed by now.
            let current_pts = requested_sample
                .map(|s| s.presentation_timestamp)
                .unwrap_or(Time::MIN);

            let requested_sample = video_description.samples.get(requested_sample_idx);
            let requested_sample_pts = requested_sample
                .map(|s| s.presentation_timestamp)
                .unwrap_or(Time::MIN);

            if requested_sample_pts < current_pts {
                re_log::trace!(
                    "Seeking backwards to sample {requested_sample_idx} (frame_nr {})",
                    requested_sample
                        .map(|s| s.frame_nr.to_string())
                        .unwrap_or("<unknown>".to_owned())
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
                self.reset(video_description)?;
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
                let last_enqueued_gop = video_description.gops.get(last_enqueued_gop_idx);
                let last_enqueued_sample_idx = last_enqueued_gop
                    .map(|gop| gop.sample_range.end)
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

            if next_gop_idx >= video_description.gops.next_index() {
                // Reached end of video with a previously enqueued GOP already.
                break;
            }

            self.enqueue_gop(video_description, next_gop_idx, video_buffers)?;
        }

        self.last_requested_sample_idx = requested_sample_idx;
        self.last_requested_gop_idx = requested_gop_idx;

        Ok(())
    }

    /// Enqueue all samples in the given GOP.
    ///
    /// Does nothing if the index is out of bounds.
    fn enqueue_gop(
        &mut self,
        video_description: &re_video::VideoDataDescription,
        gop_idx: GopIndex,
        video_buffers: &StableIndexDeque<&[u8]>,
    ) -> Result<(), VideoPlayerError> {
        let Some(gop) = video_description.gops.get(gop_idx) else {
            return Ok(());
        };

        self.last_enqueued_gop_idx = Some(gop_idx);

        re_log::trace!(
            "Enqueueing GOP {gop_idx} ({} samples)",
            gop.sample_range.len()
        );

        let samples = video_description
            .samples
            .iter_index_range_clamped(&gop.sample_range);

        for (sample_offset, sample) in samples.enumerate() {
            let sample_idx = gop.sample_range.start + sample_offset;
            let chunk = sample
                .get(video_buffers, sample_idx)
                .ok_or(VideoPlayerError::BadData)?;
            self.chunk_decoder.decode(chunk)?;
        }

        // TODO(#7484): For streaming we need to mark gops as open and do more here.
        if gop_idx + 1 == video_description.gops.next_index() {
            // Last GOP - there is nothing more to decode,
            // so flush out any pending frames:
            // See https://github.com/rerun-io/rerun/issues/8073
            self.chunk_decoder.end_of_video()?;
        }

        Ok(())
    }

    /// Reset the video decoder and discard all frames.
    pub fn reset(
        &mut self,
        video_descr: &re_video::VideoDataDescription,
    ) -> Result<(), VideoPlayerError> {
        self.chunk_decoder.reset(video_descr)?;
        self.last_requested_gop_idx = GopIndex::MAX;
        self.last_requested_sample_idx = SampleIndex::MAX;
        self.last_enqueued_gop_idx = None;
        // Do *not* reset the error state. We want to keep track of the last error.
        Ok(())
    }
}

fn update_video_texture(
    render_ctx: &RenderContext,
    chunk_decoder: &VideoSampleDecoder,
    last_error: &mut Option<TimedDecodingError>,
    video_texture: &mut VideoTexture,
    presentation_timestamp: Time,
) -> Result<(), VideoPlayerError> {
    let result =
        chunk_decoder.update_video_texture(render_ctx, video_texture, presentation_timestamp);

    match result {
        Ok(()) => {
            *last_error = None;
            Ok(())
        }
        Err(err) => {
            if matches!(err, VideoPlayerError::EmptyBuffer) {
                // No buffered frames

                // Might this be due to an error?
                //
                // We only care about decoding errors when we don't find the requested frame,
                // since we want to keep playing the video fine even if parts of it are broken.
                // That said, practically we reset the decoder and thus all frames upon error,
                // so it doesn't make a lot of difference.
                if let Some(timed_error) = last_error {
                    if DECODING_GRACE_DELAY <= timed_error.time_of_first_error.elapsed() {
                        // Report the error only if we have been in an error state for a certain amount of time.
                        // Don't immediately report the error, since we might immediately recover from it.
                        // Otherwise, this would cause aggressive flickering!
                        return Err(timed_error.latest_error.clone());
                    }
                }

                // Don't zeroed the texture, because we may just be behind on decoding
                // and showing an old frame is better than showing a blank frame,
                // because it causes "black flashes" to appear
                Ok(())
            } else {
                Err(err)
            }
        }
    }
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
