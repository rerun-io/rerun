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
use re_video::{Frame, Time};

use super::alloc_video_frame_texture;

/// Native AV1 decoder
pub struct VideoDecoder {
    data: Arc<re_video::VideoData>,
    queue: Arc<wgpu::Queue>,
    texture: GpuTexture2D,
    zeroed_texture: GpuTexture2D,
    decoder: re_video::av1::Decoder,

    frames: Arc<Mutex<Vec<Frame>>>,
    last_used_frame_timestamp: Time,
    current_segment_idx: usize,
    current_sample_idx: usize,
}

impl VideoDecoder {
    pub fn new(
        render_context: &RenderContext,
        data: Arc<re_video::VideoData>,
        _hw_acceleration: DecodeHardwareAcceleration,
    ) -> Result<Self, DecodingError> {
        let frames = Arc::new(Mutex::new(Vec::new()));

        // TODO: check that data is av1, and return error elsewise
        // TEMP: assuming `av1`, because `re_video` demuxer will panic if it's not
        let decoder = re_video::av1::Decoder::new({
            let frames = frames.clone();
            move |frame: re_video::Frame| {
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

        Ok(Self {
            data,
            queue,
            texture,
            zeroed_texture,
            decoder,

            frames,
            last_used_frame_timestamp: Time::MAX,
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
                self.enqueue_segment(requested_segment_idx + 1);
            } else {
                // forward seek by N>1 OR backward seek across segments - reset
                self.reset();
                self.enqueue_segment(requested_segment_idx);
                self.enqueue_segment(requested_segment_idx + 1);
            }
        } else if requested_sample_idx != self.current_sample_idx {
            // special case: handle seeking backwards within a single segment
            // this is super inefficient, but it's the only way to handle it
            // while maintaining a buffer of 2 segments
            let sample_distance = requested_sample_idx as isize - self.current_sample_idx as isize;
            if sample_distance < 0 {
                self.reset();
                self.enqueue_segment(requested_segment_idx);
                self.enqueue_segment(requested_segment_idx + 1);
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

        // This handles the case when we have a buffered frame that's older than the requested timestamp.
        // We don't want to show this frame to the user, because it's not actually the one they requested,
        // so instead return the last decoded frame.
        if presentation_timestamp - frame.timestamp > frame.duration {
            return Ok(VideoFrameTexture::Pending(self.texture.clone()));
        }

        if self.last_used_frame_timestamp != frame.timestamp {
            self.last_used_frame_timestamp = frame.timestamp;
            copy_video_frame_to_texture(&self.queue, frame, &self.texture.texture);
        }

        Ok(VideoFrameTexture::Ready(self.texture.clone()))
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
        let data = &self.data.data[sample.byte_offset as usize
            ..sample.byte_offset as usize + sample.byte_length as usize]; // TODO: range check

        let chunk = re_video::Chunk {
            data: data.to_vec(),
            timestamp: sample.decode_timestamp,
            duration: sample.duration,
        };

        self.decoder.decode(chunk);

        Ok(())
    }

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self) {
        self.decoder.reset();

        let mut frames = self.frames.lock();
        drop(frames.drain(..));
    }
}

fn copy_video_frame_to_texture(
    queue: &Queue,
    frame: &Frame,
    texture: &wgpu::Texture,
) -> Result<DecodingError> {
    let size = wgpu::Extent3d {
        width: frame.display_width(),
        height: frame.display_height(),
        depth_or_array_layers: 1,
    };
    let source = {
        // TODO(jan): The wgpu version we're using doesn't support `VideoFrame` yet.
        // This got fixed in https://github.com/gfx-rs/wgpu/pull/6170 but hasn't shipped yet.
        // So instead, we just pretend this is a `HtmlVideoElement` instead.
        // SAFETY: Depends on the fact that `wgpu` passes the object through as-is,
        // and doesn't actually inspect it in any way. The browser then does its own
        // typecheck that doesn't care what kind of image source wgpu gave it.
        #[allow(unsafe_code)]
        let frame = unsafe {
            std::mem::transmute::<web_sys::VideoFrame, web_sys::HtmlVideoElement>(
                frame.clone().expect("Failed to clone the video frame"),
            )
        };
        // Fake width & height to work around wgpu validating this as if it was a `HtmlVideoElement`.
        // Since it thinks this is a `HtmlVideoElement`, it will want to call `videoWidth` and `videoHeight`
        // on it to validate the size.
        // We simply redirect `displayWidth`/`displayHeight` to `videoWidth`/`videoHeight` to make it work!
        let display_width = js_sys::Reflect::get(&frame, &"displayWidth".into())
            .expect("Failed to get displayWidth property from VideoFrame.");
        js_sys::Reflect::set(&frame, &"videoWidth".into(), &display_width)
            .expect("Failed to set videoWidth property.");
        let display_height = js_sys::Reflect::get(&frame, &"displayHeight".into())
            .expect("Failed to get displayHeight property from VideoFrame.");
        js_sys::Reflect::set(&frame, &"videoHeight".into(), &display_height)
            .expect("Failed to set videoHeight property.");

        wgpu_types::ImageCopyExternalImage {
            source: wgpu_types::ExternalImageSource::HTMLVideoElement(frame),
            origin: wgpu_types::Origin2d { x: 0, y: 0 },
            flip_y: false,
        }
    };
    let dest = wgpu::ImageCopyTextureTagged {
        texture,
        mip_level: 0,
        origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
        aspect: wgpu::TextureAspect::All,
        color_space: wgpu::PredefinedColorSpace::Srgb,
        premultiplied_alpha: false,
    };
    queue.copy_external_image_to_texture(&source, dest, size);
}
