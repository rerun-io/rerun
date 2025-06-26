use std::time::Duration;

use web_time::Instant;

use re_video::{
    DecodeSettings, FrameInfo, GopIndex, SampleIndex, StableIndexDeque, Time, Timescale,
};

use super::{VideoFrameTexture, chunk_decoder::VideoSampleDecoder};
use crate::{
    RenderContext,
    resource_managers::{GpuTexture2D, SourceImageDataFormat},
    video::{VideoPlayerError, chunk_decoder::update_video_texture_with_frame},
};

/// Don't report hickups lasting shorter than this.
///
/// Delaying error reports (and showing last-good images meanwhile) allows us to skip over
/// transient errors without flickering.
///
/// Same with showing a spinner: if we show it too fast, it is annoying.
///
/// This is wallclock time and independent of how fast a video is being played back.
const DECODING_GRACE_DELAY_BEFORE_REPORTING: Duration = Duration::from_millis(400);

/// Video time duration we allow to lag behind before no longer updating the output texture.
///
/// Note that since this is video time based, not wallclock time.
/// We can't do wallclock time here since this would because we don't know the playback speed.
/// Also, hitting the tolerance limit for faster playback is desirable anyways.
const TOLLERATED_OUTPUT_DELAY: Duration = Duration::from_millis(100);

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

#[derive(Debug, Clone, Copy)]
struct SampleAndGopIndex {
    sample_idx: SampleIndex,

    /// Index of the group of pictures, that contains the sample.
    gop_idx: GopIndex,
}

/// Decode video to a texture, optimized for extracting successive frames over time.
///
/// If you want to sample multiple points in a video simultaneously, use multiple video players.
pub struct VideoPlayer {
    sample_decoder: VideoSampleDecoder,

    video_texture: VideoTexture,

    last_requested: Option<SampleAndGopIndex>,
    last_enqueued: Option<SampleAndGopIndex>,

    /// Last error that was encountered during decoding.
    ///
    /// Only fully reset after a successful decode.
    last_error: Option<TimedDecodingError>,

    /// The last time the decoder fully caught up with the frame we want to show, if ever.
    last_time_caught_up: Option<Instant>,
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

        let sample_decoder = VideoSampleDecoder::new(debug_name.clone(), |on_output| {
            re_video::new_decoder(&debug_name, description, decode_settings, on_output)
        })?;

        Ok(Self {
            sample_decoder,

            video_texture: VideoTexture {
                texture: None,
                frame_info: None,
                source_pixel_format: SourceImageDataFormat::WgpuCompatible(
                    wgpu::TextureFormat::Rgba8Unorm,
                ),
            },

            last_requested: None,
            last_enqueued: None,

            last_error: None,

            last_time_caught_up: None,
        })
    }

    /// Get the video frame at the given time stamp.
    ///
    /// This will seek in the video if needed.
    /// If you want to sample multiple points in a video simultaneously, use multiple decoders.
    ///
    /// The video data description may change over time by adding and removing samples and GOPs,
    /// but other properties are expected to be stable.
    // TODO(andreas): have to detect when decoder is playing catch-up and don't show images that we're not interested in.
    pub fn frame_at(
        &mut self,
        render_ctx: &RenderContext,
        requested_pts: Time,
        video_description: &re_video::VideoDataDescription,
        video_buffers: &StableIndexDeque<&[u8]>,
    ) -> Result<VideoFrameTexture, VideoPlayerError> {
        if requested_pts.0 < 0 {
            return Err(VideoPlayerError::NegativeTimestamp);
        }

        // Find which sample best represents the requested PTS.
        let requested_sample_idx = video_description
            .latest_sample_index_at_presentation_timestamp(requested_pts)
            .ok_or(VideoPlayerError::EmptyVideo)?;
        let requested_sample = video_description.samples.get(requested_sample_idx); // This is only `None` if we no longer have the sample around.
        let requested_pts = requested_sample
            .map(|s| s.presentation_timestamp)
            .unwrap_or(requested_pts);

        // Ensure we have enough samples enqueued to the decoder to cover the request.
        // (This method also makes sure that the next few frames become available, so call this even if we already have the frame we want.)
        self.enqueue_samples(video_description, requested_sample_idx, video_buffers)?;

        // Grab best decoded frame for the requested PTS and discard all earlier frames to save memory.
        if let Some(decoded_frame) = self
            .sample_decoder
            .latest_decoded_frame_at_and_drop_earlier_frames(requested_pts)
        {
            // Update the texture only if:
            // * we're not already up to date.
            let current_frame_info = self.video_texture.frame_info.as_ref();
            let outdated_frame =
                current_frame_info.is_none_or(|info| info.presentation_timestamp != requested_pts);

            // * the decoded frame isn't too far behind.
            //   This happens when catching up on a seek, very rapid playback or simply too slow decoding.
            //   Without this tolerance, we'd get a "rubber-band effect" in playback where we show a lot of outdated frames before (hopefully) catching up.
            //   This is especially common when seeking to the end of a large GOP since the decoder has to start from its beginning.
            let timescale = video_description.timescale.unwrap_or(Timescale::new(30)); // Assume 30 time units per second if there's no scale. As good as any guess!;
            let pts_diff = decoded_frame.info.presentation_timestamp - requested_pts;
            let not_too_far_behind = pts_diff.duration(timescale) <= TOLLERATED_OUTPUT_DELAY;

            if outdated_frame && not_too_far_behind {
                update_video_texture_with_frame(
                    render_ctx,
                    &mut self.video_texture,
                    &decoded_frame,
                )?; // Update texture errors are very unusual, error out on those immediately.

                self.video_texture.frame_info = Some(decoded_frame.info.clone());
            }

            // We apparently recovered from any errors we had previously!
            // (otherwise we wouldn't have received a frame from the decoder)
            self.last_error = None;
        };

        let current_frame_info = self.video_texture.frame_info.as_ref();
        let is_pending = self.video_texture.texture.is_none()
            || current_frame_info.is_none_or(|info| info.presentation_timestamp != requested_pts);

        // Decide whether to show a spinner or even error out.
        let show_spinner = if is_pending {
            // Might we be pending because of an error?
            if let Some(last_error) = self.last_error.as_ref() {
                // If we've been in this error state for a while now, report the error.
                // (sometimes errors are very transient and we recover from them quickly)
                if last_error.time_of_first_error.elapsed() > DECODING_GRACE_DELAY_BEFORE_REPORTING
                {
                    // Report the error only if we have been in an error state for a certain amount of time.
                    // Don't immediately report the error, since we might immediately recover from it.
                    // Otherwise, this would cause aggressive flickering!
                    return Err(last_error.latest_error.clone());
                }
            }

            self.video_texture.texture.is_none() ||
                // Show spinner if we haven't caught up in a while.
                self
                    .last_time_caught_up
                    .is_none_or(|last_time_caught_up| {
                        last_time_caught_up.elapsed() > DECODING_GRACE_DELAY_BEFORE_REPORTING
                    })
        } else {
            self.last_time_caught_up = Some(Instant::now());
            false
        };

        Ok(VideoFrameTexture {
            texture: self.video_texture.texture.clone(),
            is_pending,
            show_spinner,
            frame_info: self.video_texture.frame_info.clone(),
            source_pixel_format: self.video_texture.source_pixel_format,
        })
    }

    /// Makes sure enough samples have been enqueued to cover the requested presentation timestamp.
    fn enqueue_samples(
        &mut self,
        video_description: &re_video::VideoDataDescription,
        requested_sample_idx: SampleIndex,
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

        // Find the GOP that contains the sample.
        let requested_gop_idx = video_description
            .gop_index_containing_decode_timestamp(
                video_description.samples[requested_sample_idx].decode_timestamp,
            )
            .ok_or(VideoPlayerError::EmptyVideo)?;

        let requested = SampleAndGopIndex {
            sample_idx: requested_sample_idx,
            gop_idx: requested_gop_idx,
        };

        self.handle_errors_and_reset_decoder_if_needed(video_description, requested)?;

        // Ensure that we have as many GOPs enqueued currently as needed in order to…
        // * cover the GOP of the requested sample _plus one_ so we can always smoothly transition to the next GOP
        // * cover at least `min_num_samples_to_enqueue_ahead` samples to work around issues with some decoders
        //   (note that for large GOPs this is usually irrelevant)
        //
        // Furthermore, we have to take into account whether the current GOP got expanded since we last enqueued samples.
        // This happens regularly in live video streams.
        //
        // (potentially related to:) TODO(#7327, #7595): We don't necessarily have to enqueue full GOPs always.
        // In particularly beyond `requested_gop_idx` this can be overkill.

        if self.last_enqueued.is_none() {
            // We haven't enqueued anything so far. Enqueue the requested GOP.
            self.enqueue_gop(video_description, requested.gop_idx, video_buffers)?;
        }

        let min_last_sample_idx =
            requested_sample_idx + self.sample_decoder.min_num_samples_to_enqueue_ahead();
        loop {
            let last_enqueued = self
                .last_enqueued
                .expect("We ensured that at least one GOP was enqueued.");

            // Enqueued enough samples as described above?
            if last_enqueued.gop_idx > requested_gop_idx // Stay one GOP ahead of the requested GOP
                && last_enqueued.sample_idx >= min_last_sample_idx
            {
                break;
            }

            // Nothing more to enqueue / reached end of video?
            if last_enqueued.sample_idx + 1 == video_description.samples.next_index() {
                break;
            }

            // Retrieve the last enqueued GOP.
            let Some(last_enqueued_gop) = video_description.gops.get(last_enqueued.gop_idx) else {
                // If it no longer exist, attempt to move to the next GOP.
                self.enqueue_gop(video_description, last_enqueued.gop_idx + 1, video_buffers)?;
                continue;
            };

            // Check if the last enqueued gop is actually fully enqueued. If not, enqueue its remaining samples.
            // This happens regularly in live video streams.
            if last_enqueued.sample_idx + 1 < last_enqueued_gop.sample_range.end {
                // Enqueue all remaining samples of the current GOP.
                self.enqueue_samples_of_gop(
                    video_description,
                    last_enqueued.gop_idx,
                    &((last_enqueued.sample_idx + 1)..last_enqueued_gop.sample_range.end),
                    video_buffers,
                )?;
            } else {
                self.enqueue_gop(video_description, last_enqueued.gop_idx + 1, video_buffers)?;
            }
        }

        self.last_requested = Some(requested);

        Ok(())
    }

    #[expect(clippy::if_same_then_else)]
    fn handle_errors_and_reset_decoder_if_needed(
        &mut self,
        video_description: &re_video::VideoDataDescription,
        requested: SampleAndGopIndex,
    ) -> Result<(), VideoPlayerError> {
        // If we haven't decoded anything at all yet, reset the decoder.
        let Some(last_requested) = self.last_requested else {
            return self.reset(video_description);
        };

        // Decoding errors
        if let Some(error) = self.sample_decoder.take_error() {
            // If we already have an error set on this player, preserve its wallclock time.
            // Otherwise, set the error using the time at which it was registered.
            if let Some(last_error) = &mut self.last_error {
                last_error.latest_error = error.latest_error;
            } else {
                self.last_error = Some(error);
            }

            // For each new (!) error after entering the error state, we reset the decoder.
            // This way, it might later recover from the error as we progress in the video.
            self.reset(video_description)?;
        }
        // Seeking forward by more than one GOP
        // (starting over is more efficient than trying to have the decoder catch up)
        else if requested.gop_idx > last_requested.gop_idx.saturating_add(1) {
            self.reset(video_description)?;
        }
        // Backwards seeking across GOPs
        else if requested.gop_idx < last_requested.gop_idx {
            self.reset(video_description)?;
        }
        // Backwards seeking within the current GOP
        else if requested.sample_idx != last_requested.sample_idx {
            let requested_sample = video_description.samples.get(last_requested.sample_idx); // If it is not available, it got GC'ed by now.
            let current_pts = requested_sample
                .map(|s| s.presentation_timestamp)
                .unwrap_or(Time::MIN);

            let requested_sample = video_description.samples.get(requested.sample_idx);
            let requested_pts = requested_sample
                .map(|s| s.presentation_timestamp)
                .unwrap_or(Time::MIN);

            if requested_pts < current_pts {
                re_log::trace!(
                    "Seeking backwards to sample {} (frame_nr {})",
                    requested.sample_idx,
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

        Ok(())
    }

    /// Enqueue all samples in the given GOP.
    fn enqueue_gop(
        &mut self,
        video_description: &re_video::VideoDataDescription,
        gop_idx: GopIndex,
        video_buffers: &StableIndexDeque<&[u8]>,
    ) -> Result<(), VideoPlayerError> {
        let Some(gop) = video_description.gops.get(gop_idx) else {
            return Err(VideoPlayerError::MissingSample);
        };

        self.enqueue_samples_of_gop(video_description, gop_idx, &gop.sample_range, video_buffers)
    }

    /// Enqueues sample range *within* a GOP.
    ///
    /// All samples have to belong to the same GOP.
    fn enqueue_samples_of_gop(
        &mut self,
        video_description: &re_video::VideoDataDescription,
        gop_idx: GopIndex,
        sample_range: &std::ops::Range<SampleIndex>,
        video_buffers: &StableIndexDeque<&[u8]>,
    ) -> Result<(), VideoPlayerError> {
        debug_assert!(video_description.gops.get(gop_idx).is_some_and(|gop| {
            gop.sample_range.start <= sample_range.start && gop.sample_range.end >= sample_range.end
        }));

        for (sample_idx, sample) in video_description
            .samples
            .iter_index_range_clamped(sample_range)
        {
            let chunk = sample
                .get(video_buffers, sample_idx)
                .ok_or(VideoPlayerError::BadData)?;
            self.sample_decoder.decode(chunk)?;

            // Update continuously, since we want to keep track of our last state in case of errors.
            self.last_enqueued = Some(SampleAndGopIndex {
                sample_idx,
                gop_idx,
            });
        }

        if gop_idx + 1 == video_description.gops.next_index()
            && video_description.duration.is_some()
        {
            // Last GOP - there is nothing more to decode,
            // so flush out any pending frames:
            // See https://github.com/rerun-io/rerun/issues/8073
            self.sample_decoder.end_of_video()?;
        }

        Ok(())
    }

    /// Reset the video decoder and discard all frames.
    pub fn reset(
        &mut self,
        video_descr: &re_video::VideoDataDescription,
    ) -> Result<(), VideoPlayerError> {
        self.sample_decoder.reset(video_descr)?;
        self.last_requested = None;
        self.last_enqueued = None;
        // Do *not* reset the error state. We want to keep track of the last error.
        Ok(())
    }
}
