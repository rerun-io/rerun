// TODO(#7298): decode on native

#![allow(dead_code, unused_variables, clippy::unnecessary_wraps)]

use std::sync::Arc;

use crate::{
    resource_managers::GpuTexture2D,
    video::{DecodingError, FrameDecodingResult, VideoFrameTexture},
    RenderContext,
};

// TODO(#7298): remove `allow` once we have native video decoding
#[allow(unused_imports)]
use super::latest_at_idx;

use re_video::{Frame, Time};

use parking_lot::Mutex;
use web_time::Instant;

use super::{alloc_video_frame_texture, VideoDecoder, DECODING_ERROR_REPORTING_DELAY};

struct DecoderOutput {
    frames: Vec<Frame>,

    last_decoding_error: Option<DecodingError>,

    /// Whether we reset the decoder since the last time an error was reported.
    reset_since_last_reported_error: bool,

    /// Time at which point `last_decoding_error` changed from `None` to `Some`.
    time_when_entering_error_state: Instant,
}

impl Default for DecoderOutput {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            last_decoding_error: None,
            reset_since_last_reported_error: false,
            time_when_entering_error_state: Instant::now(),
        }
    }
}

/// Native AV1 decoder
pub struct Av1VideoDecoder {
    data: Arc<re_video::VideoData>,
    queue: Arc<wgpu::Queue>,
    texture: GpuTexture2D,
    decoder: re_video::av1::Decoder,

    decoder_output: Arc<Mutex<DecoderOutput>>,

    last_used_frame_timestamp: Time,
    current_segment_idx: usize,
    current_sample_idx: usize,
}

impl Av1VideoDecoder {
    pub fn new(
        debug_name: String,
        render_context: &RenderContext,
        data: Arc<re_video::VideoData>,
    ) -> Result<Self, DecodingError> {
        re_tracing::profile_function!();
        let full_debug_name = format!("{debug_name}, codec: {}", data.config.codec);

        if !data.config.is_av1() {
            return Err(DecodingError::UnsupportedCodec {
                codec: data.config.codec.clone(),
            });
        }

        re_log::debug!("Initializing native video decoder…");
        let decoder_output = Arc::new(Mutex::new(DecoderOutput::default()));

        let on_output = {
            let decoder_output = decoder_output.clone();
            let full_debug_name = full_debug_name.clone();
            move |frame: re_video::av1::Result<Frame>| match frame {
                Ok(frame) => {
                    re_log::trace!("Decoded frame at {:?}", frame.timestamp);
                    let mut output = decoder_output.lock();
                    output.frames.push(frame);
                    // We successfully decoded a frame, reset the error state.
                    output.last_decoding_error = None;
                }
                Err(err) => {
                    re_log::warn_once!("Error during decoding of {full_debug_name}: {err}");
                    let mut output = decoder_output.lock();
                    if output.last_decoding_error.is_none() {
                        output.time_when_entering_error_state = Instant::now();
                    }
                    output.last_decoding_error = Some(DecodingError::Decoding(err.to_string()));
                    output.reset_since_last_reported_error = false;
                }
            }
        };
        let decoder = re_video::av1::Decoder::new(full_debug_name, on_output);

        let queue = render_context.queue.clone();

        let texture = super::alloc_video_frame_texture(
            &render_context.device,
            &render_context.gpu_resources.textures,
            data.config.coded_width as u32,
            data.config.coded_height as u32,
        );

        Ok(Self {
            data,
            queue,
            texture,
            decoder,
            decoder_output,
            last_used_frame_timestamp: Time::MAX,
            current_segment_idx: usize::MAX,
            current_sample_idx: usize::MAX,
        })
    }
}

impl VideoDecoder for Av1VideoDecoder {
    fn frame_at(
        &mut self,
        render_ctx: &RenderContext,
        presentation_timestamp_s: f64,
    ) -> FrameDecodingResult {
        re_tracing::profile_function!();

        if presentation_timestamp_s < 0.0 {
            return Err(DecodingError::NegativeTimestamp);
        }
        let presentation_timestamp = Time::from_secs(presentation_timestamp_s, self.data.timescale);

        let Some(requested_segment_idx) = latest_at_idx(
            &self.data.segments,
            |segment| segment.start,
            &presentation_timestamp,
        ) else {
            return Err(DecodingError::EmptyVideo);
        };

        let Some(requested_sample_idx) = latest_at_idx(
            &self.data.samples,
            |sample| sample.decode_timestamp,
            &presentation_timestamp,
        ) else {
            return Err(DecodingError::EmptyVideo);
        };

        // Enqueue segments as needed.
        //
        // First, check for decoding errors that may have been set asynchronously and reset if it's a new error.
        {
            let decoder_output = self.decoder_output.lock();
            if decoder_output.last_decoding_error.is_some()
                && !decoder_output.reset_since_last_reported_error
            {
                // For each new (!) error after entering the error state, we reset the decoder.
                // This way, it might later recover from the error as we progress in the video.
                //
                // By resetting the current segment/sample indices, the frame enqueued code below
                // is forced to reset the decoder.
                self.current_segment_idx = usize::MAX;
                self.current_sample_idx = usize::MAX;
            }
        };
        // We maintain a buffer of 2 segments, so we can
        // always smoothly transition to the next segment.
        // We can always start decoding from any segment, because segments always begin
        // with a keyframe.
        // Backward seeks or seeks across many segments trigger a reset of the decoder,
        // because decoding all the samples between the previous sample and the requested
        // one would mean decoding and immediately discarding more frames than we otherwise
        // need to.
        if requested_segment_idx != self.current_segment_idx {
            let segment_distance =
                requested_segment_idx as isize - self.current_segment_idx as isize;
            if segment_distance == 1 {
                // forward seek to next segment - queue up the one _after_ requested
                self.enqueue_segment(requested_segment_idx + 1)?;
            } else {
                // forward seek by N>1 OR backward seek across segments - reset
                self.reset();
                self.enqueue_segment(requested_segment_idx)?;
                self.enqueue_segment(requested_segment_idx + 1)?;
            }
        } else if requested_sample_idx != self.current_sample_idx {
            // special case: handle seeking backwards within a single segment
            // this is super inefficient, but it's the only way to handle it
            // while maintaining a buffer of 2 segments
            let sample_distance = requested_sample_idx as isize - self.current_sample_idx as isize;
            if sample_distance < 0 {
                self.reset();
                self.enqueue_segment(requested_segment_idx)?;
                self.enqueue_segment(requested_segment_idx + 1)?;
            }
        }

        self.current_segment_idx = requested_segment_idx;
        self.current_sample_idx = requested_sample_idx;

        let mut decoder_output = self.decoder_output.lock();
        let frames = &mut decoder_output.frames;

        if !frames.is_empty() {
            re_log::trace_once!(
                "Looking for frame timestamp {presentation_timestamp:?} among frames {:?} - {:?}",
                frames.first().unwrap().timestamp,
                frames.last().unwrap().timestamp
            );
        }

        let Some(frame_idx) =
            latest_at_idx(frames, |frame| frame.timestamp, &presentation_timestamp)
        else {
            // No buffered frames - texture will be blank.

            // Might this be due to an error?
            //
            // We only care about decoding errors when we don't find the requested frame,
            // since we want to keep playing the video fine even if parts of it are broken.
            // That said, practically we reset the decoder and thus all frames upon error,
            // so it doesn't make a lot of difference.
            if let Some(last_decoding_error) = &decoder_output.last_decoding_error {
                if decoder_output.time_when_entering_error_state.elapsed()
                    >= DECODING_ERROR_REPORTING_DELAY
                {
                    // Report the error only if we have been in an error state for a certain amount of time.
                    // Don't immediately report the error, since we might immediately recover from it.
                    // Otherwise, this would cause aggressive flickering!
                    return Err(last_decoding_error.clone());
                }
            }

            // Don't return a zeroed texture, because we may just be behind on decoding
            // and showing an old frame is better than showing a blank frame,
            // because it causes "black flashes" to appear
            return Ok(VideoFrameTexture::Pending(self.texture.clone()));
        };

        // drain up-to (but not including) the frame idx, clearing out any frames
        // before it. this lets the video decoder output more frames.
        drop(frames.drain(0..frame_idx));

        // after draining all old frames, the next frame will be at index 0
        let frame_idx = 0;
        let frame = &frames[frame_idx];

        // This handles the case when we have a buffered frame that's older than the requested timestamp.
        // We don't want to show this frame to the user, because it's not actually the one they requested,
        // so instead return the last decoded frame.
        if presentation_timestamp - frame.timestamp > frame.duration {
            return Ok(VideoFrameTexture::Pending(self.texture.clone()));
        }

        if self.last_used_frame_timestamp != frame.timestamp {
            self.last_used_frame_timestamp = frame.timestamp;
            copy_video_frame_to_texture(&self.queue, frame, &self.texture.texture)?;
        }

        Ok(VideoFrameTexture::Ready(self.texture.clone()))
    }
}

impl Av1VideoDecoder {
    pub fn duration_ms(&self) -> f64 {
        self.data.duration_sec()
    }

    pub fn width(&self) -> u32 {
        self.data.config.coded_width as u32
    }

    pub fn height(&self) -> u32 {
        self.data.config.coded_height as u32
    }

    /// Enqueue all samples in the given segment.
    ///
    /// Does nothing if the index is out of bounds.
    fn enqueue_segment(&self, segment_idx: usize) -> Result<(), DecodingError> {
        let Some(segment) = self.data.segments.get(segment_idx) else {
            return Ok(());
        };

        let samples = &self.data.samples[segment.range()];

        // The first sample in a segment is always a key frame:
        self.enqueue_sample(&samples[0], true)?;
        for sample in &samples[1..] {
            self.enqueue_sample(sample, false)?;
        }

        Ok(())
    }

    /// Enqueue the given sample.
    fn enqueue_sample(&self, sample: &re_video::Sample, is_key: bool) -> Result<(), DecodingError> {
        let chunk = self.data.get(sample).ok_or(DecodingError::BadData)?;
        self.decoder.decode(chunk);
        Ok(())
    }

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self) {
        re_log::debug!("Resetting AV1 decoder");
        self.decoder.reset();

        let mut decoder_output = self.decoder_output.lock();
        decoder_output.reset_since_last_reported_error = true;
        decoder_output.frames.clear();

        self.current_segment_idx = usize::MAX;
        self.current_sample_idx = usize::MAX;
    }
}

fn copy_video_frame_to_texture(
    queue: &wgpu::Queue,
    frame: &Frame,
    texture: &wgpu::Texture,
) -> Result<(), DecodingError> {
    let size = wgpu::Extent3d {
        width: frame.width,
        height: frame.height,
        depth_or_array_layers: 1,
    };

    let format = match frame.format {
        re_video::PixelFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
    };

    let width_blocks = frame.width / format.block_dimensions().0;

    #[allow(clippy::unwrap_used)] // block_copy_size can only fail for weird compressed formats
    let block_size = format
        .block_copy_size(Some(wgpu::TextureAspect::All))
        .unwrap();

    let bytes_per_row_unaligned = width_blocks * block_size;

    re_tracing::profile_scope!("write_texture");
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &frame.data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row_unaligned),
            rows_per_image: None,
        },
        size,
    );

    Ok(())
}
