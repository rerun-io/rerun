use std::ops::Range;
use std::time::Duration;

use re_video::{DecodeSettings, FrameInfo, KeyframeIndex, SampleIndex, Time, VideoDeliveryMethod};
use web_time::Instant;

use super::VideoFrameTexture;
use super::chunk_decoder::VideoSampleDecoder;
use crate::resource_managers::{GpuTexture2D, SourceImageDataFormat};
use crate::video::{
    DecoderDelayState, InsufficientSampleDataError, UnloadedSampleDataError, VideoPlayerError,
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
    ///   - makes it depend on playback speed whether we hit the threshold or not
    /// * use a wall clock time based tolerance:
    ///   - any seek operation that leads to waiting for the decoder to catch up,
    ///     would cause us to show in-progress frames until the tolerance is hit
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

/// Decode video to a texture, optimized for extracting successive frames over time.
///
/// If you want to sample multiple points in a video simultaneously, use multiple video players.
pub struct VideoPlayer {
    sample_decoder: VideoSampleDecoder,

    video_texture: VideoTexture,

    last_requested: Option<SampleIndex>,
    last_enqueued: Option<SampleIndex>,

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
        re_log::trace!("Dropping VideoPlayer {:?}", self.debug_name());
    }
}

/// Looks backwards from the given sample index, and either request the first
/// missing sample we find, or stop if we find a keyframe.
///
/// This will also make sure to call `get_buffer` for all loaded samples between
/// the found index and the returned index, even if we hit an unloaded sample
/// while looking for the keyframe. This ensures that we get an opportunity to
/// mark those buffers as still being in use so that they don't get unloaded.
///
/// Returns the index of the keyframe if found.
pub fn request_keyframe_before<'a>(
    video_description: &re_video::VideoDataDescription,
    idx: SampleIndex,
    get_buffer: &dyn Fn(re_tuid::Tuid) -> &'a [u8],
) -> Result<KeyframeIndex, VideoPlayerError> {
    // Need to start from at least `samples.min_index()` since that's the index of the first sample.
    let range = video_description.samples.min_index()..idx + 1;
    if let Some((from_idx, s)) = video_description
        .samples
        .iter_index_range_clamped(&range)
        .rev()
        .find(|(_, s)| match s {
            re_video::SampleMetadataState::Present(s) => s.is_sync,
            // We don't know if this is a keyframe or not. So we stop here and wait for it
            // to be loaded.
            re_video::SampleMetadataState::Unloaded(_) => true,
        })
    {
        // Request all the sources from the unloaded/keyframe up until the current index to
        // indicate that they should stay loaded.
        for (_, sample) in video_description
            .samples
            .iter_index_range_clamped(&(from_idx..idx + 1))
        {
            get_buffer(sample.source_id());
        }

        match s {
            re_video::SampleMetadataState::Present(_) => video_description
                .keyframe_indices
                .binary_search(&from_idx)
                .map_err(|_idx| {
                    if cfg!(debug_assertions) {
                        re_log::error_once!(
                            "[DEBUG] Not tracking the keyframe at sample idx {from_idx}"
                        );
                    }
                    VideoPlayerError::BadData
                }),
            re_video::SampleMetadataState::Unloaded(_) => {
                Err(UnloadedSampleDataError::ExpectedSampleNotLoaded.into())
            }
        }
    } else {
        // If we went through all samples and didn't find any that are either unloaded or keyframes,
        // there is a keyframe missing at the start of this video.
        Err(InsufficientSampleDataError::NoKeyFramesPriorToRequestedTimestamp.into())
    }
}

/// Called if `latest_sample_index_at_presentation_timestamp` fails.
///
/// Tries to find a sample from which we can start looking for unloaded samples
/// to eventually load a keyframe before the requested pts, given that this is a
/// valid video.
fn try_request_missing_samples_at_presentation_timestamp<'a>(
    requested_pts: Time,
    video_description: &re_video::VideoDataDescription,
    get_video_buffer: &dyn Fn(re_tuid::Tuid) -> &'a [u8],
) -> VideoPlayerError {
    // Find a sample we can hook onto to start looking for keyframes.
    //
    // We always load backwards looking for a keyframe, so prefer
    // looking for samples that are after the requested timestamp. Otherwise,
    // we use the highest presentation timestamp before our given one.
    let mut best_sample_idx_before_timestamp = None;
    let sample_idx_after_timestamp =
        video_description
            .samples
            .iter_indexed()
            .find_map(|(idx, s)| {
                let s = s.sample()?;

                if s.presentation_timestamp >= requested_pts {
                    Some(idx)
                } else if best_sample_idx_before_timestamp
                    .is_none_or(|(timestamp, _)| timestamp > s.presentation_timestamp)
                {
                    best_sample_idx_before_timestamp = Some((s.presentation_timestamp, idx));
                    None
                } else {
                    None
                }
            });

    let Some(found_loaded_sample_idx) =
        sample_idx_after_timestamp.or_else(|| best_sample_idx_before_timestamp.map(|(_, idx)| idx))
    else {
        return UnloadedSampleDataError::NoLoadedSamples.into();
    };

    match request_keyframe_before(
        video_description,
        // Subtract 1 because the found sample idx could be a keyframe,
        // since the found sample is always loaded we don't have to request that.
        found_loaded_sample_idx.saturating_sub(1),
        get_video_buffer,
    ) {
        // Can end up here if the player requests a timestamp before the first sample in the video…
        Ok(_) => {
            // … which could also mean no keyframes at all, so check
            // that for a more accurate error.
            if video_description.keyframe_indices.is_empty() {
                InsufficientSampleDataError::NoKeyFrames.into()
            } else {
                InsufficientSampleDataError::NoKeyFramesPriorToRequestedTimestamp.into()
            }
        }
        Err(err) => err,
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

        if let Some(details) = description.encoding_details.as_ref()
            && let Some(bit_depth) = details.bit_depth
        {
            if bit_depth < 8 {
                re_log::warn_once!("{debug_name} has unusual bit_depth of {bit_depth}");
            } else if 8 < bit_depth {
                re_log::warn_once!(
                    "{debug_name}: HDR videos not supported. See https://github.com/rerun-io/rerun/issues/7594 for more."
                );
            }
        }

        let sample_decoder = VideoSampleDecoder::new(debug_name.clone(), |output_sender| {
            re_video::new_decoder(&debug_name, description, decode_settings, output_sender)
        })?;

        Ok(Self::new_with_encoder(sample_decoder))
    }

    pub fn new_with_encoder(sample_decoder: VideoSampleDecoder) -> Self {
        Self {
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
        }
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
    pub fn frame_at<'a>(
        &mut self,
        requested_pts: Time,
        video_description: &re_video::VideoDataDescription,
        update_video_texture_with_frame: &mut dyn FnMut(
            &mut VideoTexture,
            &re_video::Frame,
        ) -> Result<(), VideoPlayerError>,
        get_video_buffer: &dyn Fn(re_tuid::Tuid) -> &'a [u8],
    ) -> Result<VideoFrameTexture, VideoPlayerError> {
        if video_description.samples.is_empty() {
            return Err(InsufficientSampleDataError::NoSamples.into());
        }
        if requested_pts.0 < 0 {
            return Err(VideoPlayerError::NegativeTimestamp);
        }

        // Find which sample best represents the requested PTS.
        let Some(requested_sample_idx) =
            video_description.latest_sample_index_at_presentation_timestamp(requested_pts)
        else {
            self.reset(video_description)?;
            return Err(try_request_missing_samples_at_presentation_timestamp(
                requested_pts,
                video_description,
                get_video_buffer,
            ));
        };

        let requested_sample = video_description
            .samples
            .get(requested_sample_idx)
            .and_then(|s| s.sample()); // This is only `None` if we no longer have the sample around, or the sample hasn't loaded yet.
        let requested_sample_pts =
            requested_sample.map_or(requested_pts, |s| s.presentation_timestamp);

        // Ensure we have enough samples enqueued to the decoder to cover the request.
        // (This method also makes sure that the next few frames become available, so call this even if we already have the frame we want.)
        self.enqueue_samples(video_description, requested_sample_idx, get_video_buffer)?;

        // Grab best decoded frame for the requested PTS and discard all earlier frames to save memory.
        self.sample_decoder
            // Use the `requested_pts` which may be a bit higher than the PTS of the latest-at sample for `requested_pts`.
            // This is to hedge against not well-behaved decoders, that may produce PTS values that
            // don't show up in the input data (that in and on its own is a bug, but this makes it more robust)
            .process_incoming_frames_and_drop_earlier_than(requested_pts);
        if let Some(decoded_frame) = self.sample_decoder.oldest_available_frame() {
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
                update_video_texture_with_frame(&mut self.video_texture, decoded_frame)?; // Update texture errors are very unusual, error out on those immediately.
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
    fn enqueue_samples<'a>(
        &mut self,
        video_description: &re_video::VideoDataDescription,
        requested_sample_idx: SampleIndex,
        get_video_buffer: &dyn Fn(re_tuid::Tuid) -> &'a [u8],
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

        // This will request either:
        // - From the requested samples keyframe, up to `requested_sample_idx`, indicating that
        //   their sources should stay loaded.
        // - From the last unloaded sample before `requested_sample_idx`, indicating both that
        //   the unloaded sample should now be loaded, and the other samples should stay loaded.
        //
        // Meaning that if we get past this we know we have all samples between
        // the keyframe and the requested sample loaded. Which is not technically
        // required for the encoder to work if we've already enqueued the frames,
        // but it does make it more stable to still have those in-memory.
        let requested_keyframe_idx =
            request_keyframe_before(video_description, requested_sample_idx, get_video_buffer)
                .inspect_err(|_err| {
                    // We're already returning an error here.
                    let _res = self.reset(video_description);
                })?;

        self.handle_errors_and_reset_decoder_if_needed(
            video_description,
            requested_sample_idx,
            requested_keyframe_idx,
        )?;

        // Ensure that we have as many keyframes enqueued currently as needed in order to…
        // * cover the keyframe of the requested sample _plus one_ so we can always smoothly transition to the next keyframe
        // * cover at least `min_num_samples_to_enqueue_ahead` samples to work around issues with some decoders
        //   (note that for large keyframe ranges this is usually irrelevant)
        //
        // Furthermore, we have to take into account whether the current keyframe got expanded since we last enqueued samples.
        // This happens regularly in live video streams.
        //
        // (potentially related to:) TODO(#7595): We don't necessarily have to enqueue full keyframe ranges always.

        // Find the keyframe of the last enqueued sample.
        let mut keyframe_idx = if let Some(last_enqueued) = self.last_enqueued
            && let Some(keyframe_idx) = video_description.sample_keyframe_idx(last_enqueued)
        {
            if keyframe_idx < requested_keyframe_idx {
                // Skip forward and just enqueue the requested keyframe.
                self.enqueue_keyframe_range(
                    video_description,
                    requested_keyframe_idx,
                    requested_sample_idx,
                    get_video_buffer,
                )?;

                requested_keyframe_idx
            } else {
                keyframe_idx
            }
        } else {
            self.reset(video_description)?;
            // We haven't enqueued anything so far. Enqueue the requested keyframe range.
            self.enqueue_keyframe_range(
                video_description,
                requested_keyframe_idx,
                requested_sample_idx,
                get_video_buffer,
            )?;

            requested_keyframe_idx
        };

        let min_last_sample_idx =
            requested_sample_idx + self.sample_decoder.min_num_samples_to_enqueue_ahead();

        loop {
            let Some(last_enqueued) = self.last_enqueued else {
                break;
            };

            // Enqueued enough samples as described above?
            if last_enqueued >= min_last_sample_idx {
                break;
            }

            match video_description.samples.get(last_enqueued + 1) {
                Some(re_video::SampleMetadataState::Unloaded(source)) => {
                    // So far we have only requested backwards from the requested
                    // sample. This will request forward for when we're enqueueing
                    // infront of a sample.
                    get_video_buffer(*source);
                    // We require all samples and one additional we're enqueuing before the requested
                    // sample to be present.
                    //
                    // Usually `last_enqueued` is greater than `requested_sample_idx`
                    // since we stay ahead of the requested sample as described above.
                    if last_enqueued <= requested_sample_idx {
                        return Err(UnloadedSampleDataError::ExpectedSampleNotLoaded.into());
                    }

                    break;
                }
                Some(_state) => {}
                None => break,
            }

            let next_keyframe_idx = keyframe_idx + 1;
            let next_keyframe = video_description
                .keyframe_indices
                .get(next_keyframe_idx)
                .copied();

            // Check if the last enqueued keyframe range is actually fully enqueued.
            if let Some(next_keyframe) = next_keyframe
                && last_enqueued + 1 >= next_keyframe
            {
                self.enqueue_keyframe_range(
                    video_description,
                    next_keyframe_idx,
                    requested_sample_idx,
                    get_video_buffer,
                )?;

                keyframe_idx = next_keyframe_idx;
            }
            // If not, enqueue its remaining samples.
            else {
                let keyframe_range = video_description
                    .gop_sample_range_for_keyframe(keyframe_idx)
                    .ok_or(VideoPlayerError::BadData)?;

                // Ensure the keyframe stays in memory by requesting its buffer.
                if let Some(sample) = video_description.samples.get(keyframe_range.start) {
                    get_video_buffer(sample.source_id());
                }

                let range = (last_enqueued + 1)
                    ..keyframe_range.end.min(
                        requested_sample_idx
                            + self.sample_decoder.max_num_samples_to_enqueue_ahead()
                            + 1,
                    );
                self.enqueue_sample_range(video_description, &range, get_video_buffer)?;
            }
        }

        self.last_requested = Some(requested_sample_idx);

        // Signal the end of the video if we reached it.
        // This is important for some decoders to flush out all the frames.
        if !self.signaled_end_of_video
            && !treat_video_as_live_stream(&self.config, video_description)
            && self.enqueued_last_sample_of_video(video_description)
        {
            re_log::trace!("Signaling end of video");
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
            last_enqueued + 1 == video_description.samples.next_index()
        })
    }

    fn handle_errors_and_reset_decoder_if_needed(
        &mut self,
        video_description: &re_video::VideoDataDescription,
        requested: SampleIndex,
        requested_keyframe: KeyframeIndex,
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
        // Reset if our last enqueued sample has been unloaded.
        //
        // Or seeking forward by more than one GOP
        // (starting over is more efficient than trying to have the decoder catch up)
        else if self.last_enqueued.is_some_and(|enqueued_idx| {
            video_description
                .samples
                .get(enqueued_idx)
                .is_none_or(|s| s.sample().is_none())
                || enqueued_idx < video_description.keyframe_indices[requested_keyframe]
        }) {
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
        else if requested != last_requested {
            let requested_sample = video_description
                .samples
                .get(last_requested)
                .and_then(|s| s.sample()); // If it is not available, it got GC'ed by now. Or hasn't been loaded yet.
            let current_pts = requested_sample
                .map(|s| s.presentation_timestamp)
                .unwrap_or(Time::MIN);

            let requested_sample = video_description
                .samples
                .get(requested)
                .and_then(|s| s.sample());
            let requested_pts = requested_sample
                .map(|s| s.presentation_timestamp)
                .unwrap_or(Time::MIN);

            if requested_pts < current_pts {
                re_log::trace!(
                    "Seeking backwards to sample {} (frame_nr {})",
                    requested,
                    requested_sample
                        .map(|s| s.frame_nr.to_string())
                        .unwrap_or_else(|| "<unknown>".to_owned())
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

    fn enqueue_keyframe_range<'a>(
        &mut self,
        video_description: &re_video::VideoDataDescription,
        keyframe_idx: KeyframeIndex,
        requested_sample_idx: SampleIndex,
        get_video_buffer: &dyn Fn(re_tuid::Tuid) -> &'a [u8],
    ) -> Result<(), VideoPlayerError> {
        let max_last_sample_idx =
            requested_sample_idx + self.sample_decoder.max_num_samples_to_enqueue_ahead();
        let sample_range = video_description
            .gop_sample_range_for_keyframe(keyframe_idx)
            .ok_or(VideoPlayerError::BadData)?;

        if sample_range.start < max_last_sample_idx {
            let sample_range = sample_range.start..sample_range.end.min(max_last_sample_idx + 1);

            self.enqueue_sample_range(video_description, &sample_range, get_video_buffer)
        } else {
            debug_assert!(
                false,
                "[DEBUG] Tried to enqueue gop starting after max samples to enqueue"
            );
            Ok(())
        }
    }

    /// Enqueues sample range *within* a keyframe range.
    ///
    /// All samples have to belong to the same keyframe.
    fn enqueue_sample_range<'a>(
        &mut self,
        video_description: &re_video::VideoDataDescription,
        sample_range: &Range<SampleIndex>,
        get_video_buffer: &dyn Fn(re_tuid::Tuid) -> &'a [u8],
    ) -> Result<(), VideoPlayerError> {
        for (sample_idx, sample) in video_description
            .samples
            .iter_index_range_clamped(sample_range)
        {
            let sample = match sample {
                re_video::SampleMetadataState::Present(sample) => sample,
                re_video::SampleMetadataState::Unloaded(_) => {
                    return Ok(());
                }
            };
            let chunk = sample
                .get(get_video_buffer, sample_idx)
                .ok_or(VideoPlayerError::BadData)?;
            self.sample_decoder.decode(chunk)?;

            // Update continuously, since we want to keep track of our last state in case of errors.
            self.last_enqueued = Some(sample_idx);
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
        if treat_video_as_live_stream(&self.config, video_description) {
            let min_num_samples_to_enqueue_ahead =
                self.sample_decoder.min_num_samples_to_enqueue_ahead();
            let allowed_delay =
                min_num_samples_to_enqueue_ahead + self.config.tolerated_output_delay_in_num_frames;

            let sample_idx_end = video_description.samples.next_index();
            for (_, sample) in video_description.samples.iter_index_range_clamped(
                &(sample_idx_end.saturating_sub(allowed_delay + 1)..sample_idx_end),
            ) {
                let Some(sample) = sample.sample() else {
                    continue;
                };

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
fn treat_video_as_live_stream(
    config: &PlayerConfiguration,
    video_description: &re_video::VideoDataDescription,
) -> bool {
    // If this is a potentially live stream, signal the end of the video after a certain amount of time.
    // This helps decoders to flush out any pending frames.
    // (in particular the ffmpeg-executable based decoder profits from this as it tends to not emit the last 5~10 frames otherwise)
    match &video_description.delivery_method {
        VideoDeliveryMethod::Static { .. } => false,
        VideoDeliveryMethod::Stream {
            last_time_updated_samples,
        } => last_time_updated_samples.elapsed() < config.time_until_video_assumed_ended,
    }
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
