#![allow(dead_code, unused_variables, clippy::unnecessary_wraps)]

use std::sync::Arc;

use crate::{
    resource_managers::GpuTexture2D,
    video::{DecodeHardwareAcceleration, DecodingError, VideoFrameTexture},
};
use crate::{video::FrameDecodingResult, RenderContext};

// TODO(#7298): remove `allow` once we have native video decoding
#[allow(unused_imports)]
use super::latest_at_idx;

use parking_lot::Mutex;
use re_video::TimeMs;
use re_video::{decode::Frame, Time};

use super::alloc_video_frame_texture;

/// Native AV1 decoder
pub struct VideoDecoder {
    data: Arc<re_video::VideoData>,
    queue: Arc<wgpu::Queue>,
    texture: GpuTexture2D,
    zeroed_texture: GpuTexture2D,
    decoder: re_video::decode::av1::Decoder,

    frames: Arc<Mutex<Vec<Frame>>>,
    last_used_frame_timestamp: TimeMs,
    current_segment_idx: usize,
    current_sample_idx: usize,
}

impl VideoDecoder {
    pub fn new(
        render_context: &RenderContext,
        data: Arc<re_video::VideoData>,
        _hw_acceleration: DecodeHardwareAcceleration,
    ) -> Option<Self> {
        let frames = Arc::new(Mutex::new(Vec::new()));

        // TEMP: assuming `av1`, because `re_video` demuxer will panic if it's not
        let decoder = re_video::decode::av1::Decoder::new({
            let frames = frames.clone();
            move |frame: re_video::decode::Frame| {
                frames.lock().push(frame);
            }
        });

        let queue = render_context.queue.clone();

        let texture = super::alloc_video_frame_texture(
            &render_context.device,
            &render_context.gpu_resources.textures,
            data.config.coded_width as u32,
            data.config.coded_height as u32,
        );
        let zeroed_texture = alloc_video_frame_texture(
            &render_context.device,
            &render_context.gpu_resources.textures,
            data.config.coded_width as u32,
            data.config.coded_height as u32,
        );

        Some(Self {
            data,
            queue,
            texture,
            zeroed_texture,
            decoder,

            frames,
            last_used_frame_timestamp: TimeMs::new(f64::MAX),
            current_segment_idx: usize::MAX,
            current_sample_idx: usize::MAX,
        })
    }

    pub fn duration_ms(&self) -> f64 {
        self.data.duration_sec()
    }

    pub fn width(&self) -> u32 {
        self.data.config.coded_width as u32
    }

    pub fn height(&self) -> u32 {
        self.data.config.coded_height as u32
    }

    pub fn frame_at(
        &mut self,
        render_ctx: &RenderContext,
        presentation_timestamp_s: f64,
    ) -> FrameDecodingResult {
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

        // Enqueue segments as needed. We maintain a buffer of 2 segments, so we can
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
                self.enqueue_all(requested_segment_idx + 1);
            } else {
                // forward seek by N>1 OR backward seek across segments - reset
                self.reset();
                self.enqueue_all(requested_segment_idx);
                self.enqueue_all(requested_segment_idx + 1);
            }
        } else if requested_sample_idx != self.current_sample_idx {
            // special case: handle seeking backwards within a single segment
            // this is super inefficient, but it's the only way to handle it
            // while maintaining a buffer of 2 segments
            let sample_distance = requested_sample_idx as isize - self.current_sample_idx as isize;
            if sample_distance < 0 {
                self.reset();
                self.enqueue_all(requested_segment_idx);
                self.enqueue_all(requested_segment_idx + 1);
            }
        }

        self.current_segment_idx = requested_segment_idx;
        self.current_sample_idx = requested_sample_idx;

        let mut frames = self.frames.lock();

        let Some(frame_idx) =
            latest_at_idx(&frames, |frame| frame.timestamp, &presentation_timestamp)
        else {
            // No buffered frames - texture will be blank.

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

        // https://w3c.github.io/webcodecs/#output-videoframes 1. 1. states:
        //   Let timestamp and duration be the timestamp and duration from the EncodedVideoChunk associated with output.
        // we always provide both, so they should always be available
        let frame_timestamp_ms = frame.timestamp().map(TimeMs::new).unwrap_or_default();
        let frame_duration_ms = frame.duration().map(TimeMs::new).unwrap_or_default();

        // This handles the case when we have a buffered frame that's older than the requested timestamp.
        // We don't want to show this frame to the user, because it's not actually the one they requested,
        // so instead return the last decoded frame.
        if presentation_timestamp - frame_timestamp_ms > frame_duration_ms {
            return Ok(VideoFrameTexture::Pending(self.texture.clone()));
        }

        if self.last_used_frame_timestamp != frame_timestamp_ms {
            copy_video_frame_to_texture(&self.queue, frame, &self.texture.texture);
            self.last_used_frame_timestamp = frame_timestamp_ms;
        }

        self.texture.clone()
    }

    /// Enqueue all samples in the given segment.
    ///
    /// Does nothing if the index is out of bounds.
    fn enqueue_all(&self, segment_idx: usize) {
        let Some(segment) = self.data.segments.get(segment_idx) else {
            return;
        };

        self.enqueue(&segment.samples[0], true);
        for sample in &segment.samples[1..] {
            self.enqueue(sample, false);
        }
    }

    /// Enqueue the given sample.
    fn enqueue(&self, sample: &re_video::Sample, is_key: bool) {
        let data = Uint8Array::from(&self.data.get(sample));
        let type_ = if is_key {
            EncodedVideoChunkType::Key
        } else {
            EncodedVideoChunkType::Delta
        };
        let chunk = EncodedVideoChunkInit::new(&data, sample.timestamp.as_f64(), type_);
        chunk.set_duration(sample.duration.as_f64());
        let Some(chunk) = EncodedVideoChunk::new(&chunk)
            .inspect_err(|err| {
                re_log::error!("failed to create video chunk: {}", js_error_to_string(err));
            })
            .ok()
        else {
            return;
        };

        self.decoder.decode(&chunk);
    }

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self) {
        self.decoder.reset();

        let mut frames = self.frames.lock();
        drop(frames.drain(..));
    }
}
