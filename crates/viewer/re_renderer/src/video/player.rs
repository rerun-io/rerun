use std::time::Duration;

use web_time::Instant;

use re_video::{DecodeSettings, FrameInfo, GopIndex, SampleIndex, StableIndexDeque, Time};

use super::{VideoFrameTexture, chunk_decoder::VideoSampleDecoder};
use crate::{
    RenderContext,
    resource_managers::{GpuTexture2D, SourceImageDataFormat},
    video::{DecoderDelayState, VideoPlayerError, chunk_decoder::update_video_texture_with_frame},
};

pub struct PlayerConfiguration {
    /// Don't report hickups lasting shorter than this.
    ///
    /// Delaying error reports (and showing last-good images meanwhile) allows us to skip over
    /// transient errors without flickering.
    ///
    /// Same with showing a spinner: if we show it too fast, it is annoying.
    ///
    /// This is wallclock time and independent of how fast a video is being played back.
    pub decoding_grace_delay_before_reporting: Duration,

    /// Number of frames we allow to lag behind before no longer updating the output texture.
    ///
    /// This a number of frames on the presentation timeline and independent of
    /// sample order for decoding purposes.
    ///
    /// Discarded alternatives:
    /// * use video time based tolerance:
    ///   -> makes it depend on playback speed whether we hit the threshold or not
    /// * use a wall clock time based tolerance:
    ///   -> any seek operation that leads to waiting for the decoder to catch up,
    ///      would cause us to show in-progress frames until the tolerance is hit
    pub tolerated_output_delay_in_num_frames: usize,

    /// If we haven't seen new samples in this amount of time, we assume the video has ended
    /// and signal the end of the video to the decoder.
    pub time_until_video_assumed_ended: Duration,
}

impl Default for PlayerConfiguration {
    fn default() -> Self {
        Self {
            decoding_grace_delay_before_reporting: Duration::from_millis(400),
            tolerated_output_delay_in_num_frames: 3,
            time_until_video_assumed_ended: Duration::from_millis(250),
        }
    }
}

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

    /// Whether we've signaled the end of the video to the decoder since the last decoder reset.
    signaled_end_of_video: bool,

    /// Last error that was encountered during decoding.
    ///
    /// Only fully reset after a successful decode.
    last_error: Option<TimedDecodingError>,

    /// The last time the decoder fully caught up with the frame we want to show, if ever.
    last_time_caught_up: Option<Instant>,

    /// Tracks whether we're waiting for the decoder to catch up or not.
    decoder_delay_state: DecoderDelayState,

    config: PlayerConfiguration,
}

impl re_byte_size::SizeBytes for VideoPlayer {
    fn heap_size_bytes(&self) -> u64 {
        self.sample_decoder.heap_size_bytes()
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        re_log::debug!("Dropping VideoPlayer {:?}", self.debug_name());
    }
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

            signaled_end_of_video: false,

            last_error: None,

            last_time_caught_up: None,
            decoder_delay_state: DecoderDelayState::UpToDate,

            config: PlayerConfiguration::default(),
        })
    }

    pub fn debug_name(&self) -> &str {
        self.sample_decoder.debug_name()
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
        let requested_sample_pts =
            requested_sample.map_or(requested_pts, |s| s.presentation_timestamp);

        // Ensure we have enough samples enqueued to the decoder to cover the request.
        // (This method also makes sure that the next few frames become available, so call this even if we already have the frame we want.)
        self.enqueue_samples(video_description, requested_sample_idx, video_buffers)?;

        // Grab best decoded frame for the requested PTS and discard all earlier frames to save memory.
        if let Some(decoded_frame) = self
            .sample_decoder
            // Use the `requested_pts` which may be a bit higher than the PTS of the latest-at sample for `requested_pts`.
            // This is to hedge against not well-behaved decoders, that may produce PTS values that
            // don't show up in the input data (that in and on its own is a bug, but this makes it more robust)
            .latest_decoded_frame_at_and_drop_earlier_frames(requested_pts)
        {
            self.decoder_delay_state = self.determine_new_decoder_delay_state(
                video_description,
                requested_sample,
                decoded_frame.info.presentation_timestamp,
            );

            // Update the texture if it isn't already up to date and we're not waiting for the decoder to catch up.
            let current_frame_info = self.video_texture.frame_info.as_ref();
            if current_frame_info
                .is_none_or(|info| info.presentation_timestamp != requested_sample_pts)
                && self.decoder_delay_state != DecoderDelayState::Behind
            {
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
        } else {
            // If the sample decoder didn't report a frame we naturally still use the last video texture.
            // This texture may or may not be up to date, update the delay state accordingly!
            let current_frame_info = self.video_texture.frame_info.as_ref();
            self.decoder_delay_state = if let Some(last_decoded_pts) =
                current_frame_info.map(|info| info.presentation_timestamp)
            {
                self.determine_new_decoder_delay_state(
                    video_description,
                    requested_sample,
                    last_decoded_pts,
                )
            } else {
                DecoderDelayState::Behind
            };
        }

        // Decide whether to show a spinner or even error out.
        let show_spinner = match self.decoder_delay_state {
            DecoderDelayState::UpToDate => {
                self.last_time_caught_up = Some(Instant::now());
                false
            }

            // Haven't caught up, but intentionally don't show a spinner.
            DecoderDelayState::UpToDateToleratedEdgeOfLiveStream => false,

            DecoderDelayState::UpToDateWithinTolerance | DecoderDelayState::Behind => {
                // Might we be pending because of an error?
                if let Some(last_error) = self.last_error.as_ref() {
                    // If we've been in this error state for a while now, report the error.
                    // (sometimes errors are very transient and we recover from them quickly)
                    if last_error.time_of_first_error.elapsed()
                        > self.config.decoding_grace_delay_before_reporting
                    {
                        // Report the error only if we have been in an error state for a certain amount of time.
                        // Don't immediately report the error, since we might immediately recover from it.
                        // Otherwise, this would cause aggressive flickering!
                        return Err(last_error.latest_error.clone());
                    }
                }

                self.video_texture.texture.is_none()
                    || self.last_time_caught_up.is_none_or(|last_time_caught_up| {
                        last_time_caught_up.elapsed()
                            > self.config.decoding_grace_delay_before_reporting
                    })
            }
        };

        Ok(VideoFrameTexture {
            texture: self.video_texture.texture.clone(),
            decoder_delay_state: self.decoder_delay_state,
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
        // (potentially related to:) TODO(#7595): We don't necessarily have to enqueue full GOPs always.
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

        // Signal the end of the video if we reached it.
        // This is important for some decoders to flush out all the frames.
        if !self.signaled_end_of_video
            && treat_video_as_finite(&self.config, video_description)
            && self.enqueued_last_sample_of_video(video_description)
        {
            re_log::debug!("Signaling end of video");
            self.signaled_end_of_video = true;
            self.sample_decoder.end_of_video()?;
        }

        Ok(())
    }

    fn enqueued_last_sample_of_video(
        &self,
        video_description: &re_video::VideoDataDescription,
    ) -> bool {
        self.last_enqueued.is_some_and(|last_enqueued| {
            last_enqueued.sample_idx + 1 == video_description.samples.next_index()
        })
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
        // Previously signaled the end of the video, but encountering frames that are newer than the last enqueued.
        else if self.signaled_end_of_video
            && !self.enqueued_last_sample_of_video(video_description)
        {
            re_log::debug!(
                "Reset because new frames appeared since we previously signaled the end of video."
            );
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
        self.signaled_end_of_video = false;
        // Do *not* reset the error state. We want to keep track of the last error.
        Ok(())
    }

    /// Given the current decoder delay state, update it based on the new requested frame and the last decoded frame.
    #[must_use]
    fn determine_new_decoder_delay_state(
        &self,
        video_description: &re_video::VideoDataDescription,
        requested_sample: Option<&re_video::SampleMetadata>,
        last_decoded_frame_pts: Time,
    ) -> DecoderDelayState {
        let Some(requested_sample) = requested_sample else {
            // Desired sample doesn't exist. This should only happen if the video is being GC'ed from the back.
            // We're technically not catching up, but we may as well behave as if we are.
            return DecoderDelayState::Behind;
        };

        if requested_sample.presentation_timestamp == last_decoded_frame_pts {
            return DecoderDelayState::UpToDate;
        }

        // If we're streaming in live video, we're a bit more relaxed about what counts as "catching up" for newly incoming frames:
        // * we don't want to show the spinner too eagerly and rather give the impression of a delayed stream
        // * some decoders need a certain amount of samples in the queue to produce a frame.
        //   See AsyncDecoder::min_num_samples_to_enqueue_ahead for more details about decoder peculiarities.
        let recently_updated_video = video_description
            .last_time_updated_samples
            .is_some_and(|t| t.elapsed() < self.config.time_until_video_assumed_ended);
        if recently_updated_video {
            let min_num_samples_to_enqueue_ahead =
                self.sample_decoder.min_num_samples_to_enqueue_ahead();
            let allowed_delay =
                min_num_samples_to_enqueue_ahead + self.config.tolerated_output_delay_in_num_frames;

            let sample_idx_end = video_description.samples.next_index();
            for (_, sample) in video_description.samples.iter_index_range_clamped(
                &(sample_idx_end.saturating_sub(allowed_delay + 1)..sample_idx_end),
            ) {
                if sample.presentation_timestamp <= last_decoded_frame_pts {
                    return DecoderDelayState::UpToDateToleratedEdgeOfLiveStream;
                }
            }
        }

        match self.decoder_delay_state {
            DecoderDelayState::UpToDate
            | DecoderDelayState::UpToDateWithinTolerance
            | DecoderDelayState::UpToDateToleratedEdgeOfLiveStream => {
                if is_significantly_behind(
                    video_description,
                    requested_sample,
                    last_decoded_frame_pts,
                    self.config.tolerated_output_delay_in_num_frames,
                ) {
                    DecoderDelayState::Behind
                } else {
                    DecoderDelayState::UpToDateWithinTolerance
                }
            }

            DecoderDelayState::Behind => {
                // Only exit behind state if we caught up to the requested frame.
                DecoderDelayState::Behind
            }
        }
    }
}

/// Whether we should assume the video has a defined end and won't add new samples.
///
/// Note that we need to be robust against this being wrong and the video getting new samples in the future after all.
/// The result should be treated as a heuristic.
fn treat_video_as_finite(
    config: &PlayerConfiguration,
    video_description: &re_video::VideoDataDescription,
) -> bool {
    // If this is a potentially live stream, signal the end of the video after a certain amount of time.
    // This helps decoders to flush out any pending frames.
    // (in particular the ffmpeg-executable based decoder profits from this as it tends to not emit the last 5~10 frames otherwise)
    video_description.duration.is_some()
        || video_description
            .last_time_updated_samples
            .is_some_and(|last_time_updated_samples| {
                last_time_updated_samples.elapsed() > config.time_until_video_assumed_ended
            })
}

/// Determine whether the decoder is catching up with the requested frame within a certain tolerance.
fn is_significantly_behind(
    video_description: &re_video::VideoDataDescription,
    requested_sample: &re_video::SampleMetadata,
    decoded_frame_pts: Time,
    tolerated_output_delay_in_num_frames: usize,
) -> bool {
    let requested_pts = requested_sample.presentation_timestamp;

    if decoded_frame_pts == requested_pts {
        // Decoder caught up with request!
        return false;
    }

    if decoded_frame_pts > requested_pts {
        // We did a backwards seek and haven't decoded a single frame since then.
        return true;
    }

    // Decoder did not produce the desired frame, but something _before_ the requested frame.
    // Figure out how many frames we're behind. If this is higher than a certain tolerance, don't report it as catching up.
    //
    // Note that this can happen either because:
    // * we did a non-trivial seek operation and are waiting for the decoder to catch up, showing between results would be irritating
    // * the decoder is not fast enough to keep up with playback, i.e. we'll never catch up so anything we show will always be wrong
    //
    // Since frames aren't in presentation time order and may have varying durations (i.e. the video has variable frame rate),
    // we have to successively use `latest_sample_index_at_presentation_timestamp`:
    // Start at the desired sample and walk backwards from there until we find the sample for the actually produced frame.
    let mut num_frames_behind = 0;
    let mut sample = Some(requested_sample);
    loop {
        let Some(current_sample) = sample else {
            // Sample doesn't exist anymore. This should only happen if the video is being GC'ed from the back.
            // We're technically not catching up, but we may as well behave as if we are.
            return true;
        };
        if current_sample.presentation_timestamp <= decoded_frame_pts {
            // Decoded PTS is _supposed_ to show be exactly matched with one of the sample PTS.
            // Checking for smaller equal here, hedges against bugs in decoders that may emit PTS values
            // that don't show up in the input data.
            if current_sample.presentation_timestamp != decoded_frame_pts {
                re_log::debug!(
                    "PTS {:?} of decoded sample is not equal to any sample pts {:?}. This hints at a bug in the decoder implementation.",
                    decoded_frame_pts,
                    current_sample.presentation_timestamp
                );
            }

            // This is the frame we actually got and we stayed under the tolerance.
            // This may happen if the load on the decoder fluctuates or it is just about able to keep up with playback.
            return false;
        }

        num_frames_behind += 1;
        if num_frames_behind > tolerated_output_delay_in_num_frames {
            return true;
        }

        // Check the sample prior to this one.
        sample = video_description.previous_presented_sample(current_sample);
    }
}
