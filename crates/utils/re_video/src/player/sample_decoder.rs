use std::collections::BTreeMap;

use crate::{Chunk, Frame, Receiver, Sender, Time, VideoDataDescription};

use super::{TimedDecodingError, VideoPlayerError};

#[derive(Default)]
pub(super) struct DecoderOutput {
    /// Frames sorted by PTS.
    ///
    /// *Almost* all decoders are outputting frames in presentation timestamp order.
    /// However, WebCodec decoders on Firefox & Safari have been observed to output frames in decode order.
    /// (i.e. the order in which they were submitted)
    /// Therefore, we have to be careful not to assume that an incoming frame isn't in the past even on a freshly
    /// reset decoder.
    /// See also <https://github.com/rerun-io/rerun/issues/7961>
    ///
    /// Note that this technically a bug in their respective WebCodec implementations as the spec says
    /// (<https://www.w3.org/TR/webcodecs/#dom-videodecoder-decode>):
    /// `VideoDecoder` requires that frames are output in the order they expect to be presented, commonly known as presentation order.
    /// Either way, being robust against this seems like a good idea!
    frames_by_pts: BTreeMap<Time, Frame>,

    /// Set on error; reset on success.
    error: Option<TimedDecodingError>,
}

impl DecoderOutput {
    fn clear(&mut self) {
        self.error = None;
        self.frames_by_pts.clear();
    }
}

impl re_byte_size::SizeBytes for DecoderOutput {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            frames_by_pts,
            error: _,
        } = self;
        frames_by_pts.heap_size_bytes()
    }
}

/// Internal implementation detail of the [`super::VideoPlayer`].
///
/// Expected to be reset upon backwards seeking.
pub struct VideoSampleDecoder {
    debug_name: String,
    decoder: Box<dyn crate::AsyncDecoder>,

    frame_receiver: Receiver<crate::FrameResult>,
    decoder_output: DecoderOutput,

    /// The [`Chunk::sample_idx`] of the latest submitted sample.
    latest_sample_idx: Option<usize>,
}

impl re_byte_size::SizeBytes for VideoSampleDecoder {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            debug_name,
            decoder: _,        // TODO(emilk): maybe we should count this
            frame_receiver: _, // TODO(RR-3366): we should definitely count this
            decoder_output,
            latest_sample_idx: _,
        } = self;
        debug_name.heap_size_bytes() + decoder_output.heap_size_bytes()
    }
}

impl VideoSampleDecoder {
    pub fn new(
        debug_name: String,
        make_decoder: impl FnOnce(
            Sender<crate::FrameResult>,
        ) -> crate::DecodeResult<Box<dyn crate::AsyncDecoder>>,
    ) -> Result<Self, VideoPlayerError> {
        re_tracing::profile_function!();

        let (decoder_output_sender, frame_receiver) =
            crate::channel(format!("{debug_name}-VideoSampleDecoder"));
        let decoder = make_decoder(decoder_output_sender)?;

        Ok(Self {
            debug_name,
            decoder,
            decoder_output: DecoderOutput::default(),
            frame_receiver,
            latest_sample_idx: None,
        })
    }

    /// Processes all frames received from the asynchronously running decoder.
    fn process_decoder_output(&mut self) {
        loop {
            match self.frame_receiver.try_recv() {
                Ok(frame) => {
                    match frame {
                        Ok(frame) => {
                            re_log::trace!(
                                "Decoded frame at PTS {:?}",
                                frame.info.presentation_timestamp
                            );
                            self.decoder_output
                                .frames_by_pts
                                .insert(frame.info.presentation_timestamp, frame);
                            self.decoder_output.error = None; // We successfully decoded a frame, reset the error state.
                        }
                        Err(err) => {
                            // Many of the errors we get from a decoder are recoverable.
                            // They may be very frequent, but it's still useful to see them in the debug log for troubleshooting.
                            re_log::debug!("Error during decoding of {}: {err}", self.debug_name);

                            let err = VideoPlayerError::Decoding(err);
                            if let Some(error) = &mut self.decoder_output.error {
                                error.latest_error = err;
                            } else {
                                self.decoder_output.error = Some(TimedDecodingError::new(err));
                            }
                        }
                    }
                }

                Err(crate::TryRecvError::Empty) => {
                    break;
                }

                Err(crate::TryRecvError::Disconnected) => {
                    self.decoder_output.error = Some(TimedDecodingError::new(
                        VideoPlayerError::DecoderUnexpectedlyExited,
                    ));
                    break;
                }
            }
        }
    }

    pub fn debug_name(&self) -> &str {
        &self.debug_name
    }

    /// Start decoding the given chunk.
    pub fn decode(&mut self, chunk: Chunk) -> Result<(), VideoPlayerError> {
        let sample_idx = chunk.sample_idx;

        if let Some(latest_sample_idx) = self.latest_sample_idx {
            // Some sanity checks:
            if latest_sample_idx + 1 == sample_idx {
                // All good!
            } else if latest_sample_idx < sample_idx {
                return Err(super::InsufficientSampleDataError::MissingSamples.into());
            } else if sample_idx == latest_sample_idx {
                return Err(super::InsufficientSampleDataError::DuplicateSampleIdx.into());
            } else {
                return Err(super::InsufficientSampleDataError::OutOfOrderSampleIdx.into());
            }
        }

        self.decoder.submit_chunk(chunk)?;

        self.latest_sample_idx = Some(sample_idx);

        Ok(())
    }

    /// Called after submitting the last chunk.
    ///
    /// Should flush all pending frames.
    pub fn end_of_video(&mut self) -> Result<(), VideoPlayerError> {
        self.decoder.end_of_video()?;
        self.latest_sample_idx = None;
        Ok(())
    }

    /// Minimum number of samples the decoder requests to stay head of the currently requested sample.
    ///
    /// I.e. if sample N is requested, then the encoder would like to see at least all the samples from
    /// [start of N's GOP] until [N + `min_num_samples_to_enqueue_ahead`].
    /// Codec specific constraints regarding what samples can be decoded (samples may depend on other samples in their GOP)
    /// still apply independently of this.
    ///
    /// This can be used as a workaround for decoders that are known to need additional samples to produce outputs.
    pub fn min_num_samples_to_enqueue_ahead(&self) -> usize {
        self.decoder.min_num_samples_to_enqueue_ahead()
    }

    pub fn max_num_samples_to_enqueue_ahead(&self) -> usize {
        // To not fill memory up too much, only queue up a limited amount of samples.
        //
        // 25 here is arbitrary so far, but seems to work well with the encoder
        // giving back frames and not waiting for a secondary keyframe.
        self.min_num_samples_to_enqueue_ahead() + 25
    }

    /// Returns the latest decoded frame at the given PTS and drops all earlier frames than the given PTS.
    ///
    /// Afterwards, you can retrieve the frame that is at or after the PTS using [`Self::oldest_available_frame`]
    /// (without a mutable reference to the decoder).
    pub fn process_incoming_frames_and_drop_earlier_than(&mut self, pts: Time) {
        self.process_decoder_output();

        // Latest-at semantics means that if `pts` doesn't land on the exact PTS of a decode frame we have,
        // we provide the next *older* frame.
        let frames_by_pts = &mut self.decoder_output.frames_by_pts;
        let latest_at_pts = frames_by_pts
            .range(..=pts)
            .next_back()
            .map_or(pts, |(k, _v)| *k);

        // Keep everything at or after the given PTS.
        *frames_by_pts = frames_by_pts.split_off(&latest_at_pts);
    }

    /// Returns the latest decoded frame.
    pub fn oldest_available_frame(&self) -> Option<&Frame> {
        self.decoder_output
            .frames_by_pts
            .first_key_value()
            .map(|(_, v)| v)
    }

    /// Reset the video decoder and discard all frames.
    pub fn reset(&mut self, video_descr: &VideoDataDescription) -> Result<(), VideoPlayerError> {
        self.decoder.reset(video_descr)?;

        // Flush out any pending frames.
        self.process_decoder_output();
        self.decoder_output.clear();
        self.latest_sample_idx = None;

        Ok(())
    }

    /// Return and clear the latest error that happened during decoding.
    pub fn take_error(&mut self) -> Option<TimedDecodingError> {
        self.decoder_output.error.take()
    }
}
