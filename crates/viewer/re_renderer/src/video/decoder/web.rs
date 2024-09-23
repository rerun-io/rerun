use std::sync::Arc;

use js_sys::{Function, Uint8Array};
use parking_lot::Mutex;
use re_video::Time;
use wasm_bindgen::{closure::Closure, JsCast as _};
use web_sys::{
    EncodedVideoChunk, EncodedVideoChunkInit, EncodedVideoChunkType, VideoDecoderConfig,
    VideoDecoderInit,
};

use super::latest_at_idx;
use crate::{
    resource_managers::GpuTexture2D,
    video::{DecodingError, FrameDecodingResult},
    RenderContext,
};

#[derive(Clone)]
#[repr(transparent)]
struct VideoFrame(web_sys::VideoFrame);

impl Drop for VideoFrame {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl std::ops::Deref for VideoFrame {
    type Target = web_sys::VideoFrame;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct BufferedFrame {
    /// Time at which the frame appears in the video stream.
    composition_timestamp: Time,

    duration: Time,

    inner: VideoFrame,
}

pub struct VideoDecoder {
    data: Arc<re_video::VideoData>,
    queue: Arc<wgpu::Queue>,
    texture: GpuTexture2D,

    decoder: web_sys::VideoDecoder,

    frames: Arc<Mutex<Vec<BufferedFrame>>>,
    last_used_frame_timestamp: Time,
    current_segment_idx: usize,
    current_sample_idx: usize,
}

// SAFETY: There is no way to access the same JS object from different OS threads
//         in a way that could result in a data race.

#[allow(unsafe_code)]
// Clippy did not recognize a safety comment on these impls no matter what I tried:
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe impl Send for VideoDecoder {}

#[allow(unsafe_code)]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe impl Sync for VideoDecoder {}

#[allow(unsafe_code)]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe impl Send for VideoFrame {}

#[allow(unsafe_code)]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe impl Sync for VideoFrame {}

impl Drop for VideoDecoder {
    fn drop(&mut self) {
        if let Err(err) = self.decoder.close() {
            re_log::warn!(
                "Error when closing video decoder: {}",
                js_error_to_string(&err)
            );
        }
    }
}

impl VideoDecoder {
    pub fn new(
        render_context: &RenderContext,
        data: Arc<re_video::VideoData>,
    ) -> Result<Self, DecodingError> {
        let frames = Arc::new(Mutex::new(Vec::with_capacity(16)));
        let timescale = data.timescale;

        let decoder = init_video_decoder({
            let frames = frames.clone();
            move |frame: web_sys::VideoFrame| {
                let composition_timestamp =
                    Time::from_micros(frame.timestamp().unwrap_or(0.0), timescale);
                let duration = Time::from_micros(frame.duration().unwrap_or(0.0), timescale);
                let frame = VideoFrame(frame);
                frames.lock().push(BufferedFrame {
                    composition_timestamp,
                    duration,
                    inner: frame,
                });
            }
        })?;

        let queue = render_context.queue.clone();

        // NOTE: both textures are assumed to be rgba8unorm
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

            frames,
            last_used_frame_timestamp: Time::new(u64::MAX),
            current_segment_idx: usize::MAX,
            current_sample_idx: usize::MAX,
        })
    }

    pub fn frame_at(&mut self, timestamp_s: f64) -> FrameDecodingResult {
        if timestamp_s < 0.0 {
            return FrameDecodingResult::Error(DecodingError::NegativeTimestamp);
        }
        let timescale = self.data.timescale;
        let timestamp = Time::from_secs(timestamp_s, timescale);

        let Some(requested_segment_idx) =
            latest_at_idx(&self.data.segments, |segment| segment.start, &timestamp)
        else {
            return FrameDecodingResult::Error(DecodingError::EmptyVideo);
        };
        let requested_segment = &self.data.segments[requested_segment_idx];

        let Some(requested_sample_idx) = latest_at_idx(
            &self.data.samples[requested_segment.range()],
            |sample| sample.decode_timestamp,
            &timestamp,
        ) else {
            // This should never happen, because segments are never empty.
            return FrameDecodingResult::Error(DecodingError::EmptySegment);
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
            let segment_distance = requested_segment_idx.checked_sub(self.current_segment_idx);
            if segment_distance == Some(1) {
                // forward seek to next segment - queue up the one _after_ requested
                self.enqueue_segment(requested_segment_idx + 1);
            } else {
                // Startup, forward seek by N>1, or backward seek across segments -> reset decoder
                if let Err(err) = self.reset() {
                    return FrameDecodingResult::Error(err);
                }
                self.enqueue_segment(requested_segment_idx);
                self.enqueue_segment(requested_segment_idx + 1);
            }
        } else if requested_sample_idx != self.current_sample_idx {
            // special case: handle seeking backwards within a single segment
            // this is super inefficient, but it's the only way to handle it
            // while maintaining a buffer of 2 segments
            let sample_distance = requested_sample_idx as isize - self.current_sample_idx as isize;
            if sample_distance < 0 {
                if let Err(err) = self.reset() {
                    return FrameDecodingResult::Error(err);
                }
                self.enqueue_segment(requested_segment_idx);
                self.enqueue_segment(requested_segment_idx + 1);
            }
        }

        self.current_segment_idx = requested_segment_idx;
        self.current_sample_idx = requested_sample_idx;

        let mut frames = self.frames.lock();

        let Some(frame_idx) =
            latest_at_idx(&frames, |frame| frame.composition_timestamp, &timestamp)
        else {
            // no buffered frames - texture will be blank
            // Don't return a zeroed texture, because we may just be behind on decoding
            // and showing an old frame is better than showing a blank frame,
            // because it causes "black flashes" to appear
            return FrameDecodingResult::Pending(self.texture.clone());
        };

        // drain up-to (but not including) the frame idx, clearing out any frames
        // before it. this lets the video decoder output more frames.
        drop(frames.drain(0..frame_idx));

        // after draining all old frames, the next frame will be at index 0
        let frame_idx = 0;
        let frame = &frames[frame_idx];

        let frame_timestamp_ms = frame.composition_timestamp.into_millis(timescale);
        let frame_duration_ms = frame.duration.into_millis(timescale);

        // This handles the case when we have a buffered frame that's older than the requested timestamp.
        // We don't want to show this frame to the user, because it's not actually the one they requested,
        // so instead return the last decoded frame.
        if timestamp.into_millis(timescale) - frame_timestamp_ms > frame_duration_ms {
            return FrameDecodingResult::Pending(self.texture.clone());
        }

        if self.last_used_frame_timestamp != frame.composition_timestamp {
            self.last_used_frame_timestamp = frame.composition_timestamp;
            copy_video_frame_to_texture(&self.queue, &frame.inner, &self.texture.texture);
        }

        FrameDecodingResult::Ready(self.texture.clone())
    }

    /// Enqueue all samples in the given segment.
    ///
    /// Does nothing if the index is out of bounds.
    fn enqueue_segment(&self, segment_idx: usize) {
        let Some(segment) = self.data.segments.get(segment_idx) else {
            return;
        };

        let samples = &self.data.samples[segment.range()];

        // The first sample in a segment is always a key frame:
        self.enqueue_sample(&samples[0], true);
        for sample in &samples[1..] {
            self.enqueue_sample(sample, false);
        }
    }

    /// Enqueue the given sample.
    fn enqueue_sample(&self, sample: &re_video::Sample, is_key: bool) {
        let data = Uint8Array::from(
            &self.data.data[sample.byte_offset as usize
                ..sample.byte_offset as usize + sample.byte_length as usize],
        );
        let type_ = if is_key {
            EncodedVideoChunkType::Key
        } else {
            EncodedVideoChunkType::Delta
        };
        // TODO(jan): use `composition_timestamp` instead
        let chunk = EncodedVideoChunkInit::new(
            &data,
            sample.decode_timestamp.into_micros(self.data.timescale),
            type_,
        );
        chunk.set_duration(sample.duration.into_micros(self.data.timescale));
        let Some(chunk) = EncodedVideoChunk::new(&chunk)
            .inspect_err(|err| {
                // TODO(#7373): return this error once the decoder tries to return a frame for this sample. how exactly?
                re_log::error!("failed to create video chunk: {}", js_error_to_string(err));
            })
            .ok()
        else {
            return;
        };

        if let Err(err) = self.decoder.decode(&chunk) {
            // TODO(#7373): return this error once the decoder tries to return a frame for this sample. how exactly?
            re_log::error!("Failed to decode video chunk: {}", js_error_to_string(&err));
        }
    }

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self) -> Result<(), DecodingError> {
        self.decoder
            .reset()
            .map_err(|err| DecodingError::ResetFailure(js_error_to_string(&err)))?;
        self.decoder
            .configure(&js_video_decoder_config(&self.data.config))
            .map_err(|err| DecodingError::ConfigureFailure(js_error_to_string(&err)))?;

        let mut frames = self.frames.lock();
        drop(frames.drain(..));

        Ok(())
    }
}

fn copy_video_frame_to_texture(
    queue: &wgpu::Queue,
    frame: &web_sys::VideoFrame,
    texture: &wgpu::Texture,
) {
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

fn init_video_decoder(
    on_output: impl Fn(web_sys::VideoFrame) + 'static,
) -> Result<web_sys::VideoDecoder, DecodingError> {
    let on_output = Closure::wrap(Box::new(on_output) as Box<dyn Fn(web_sys::VideoFrame)>);
    let on_error = Closure::wrap(Box::new(|err: js_sys::Error| {
        // TODO(#7373): store this error and report during decode
        let err = std::string::ToString::to_string(&err.to_string());
        re_log::error!("failed to decode video: {err}");
    }) as Box<dyn Fn(js_sys::Error)>);

    let Ok(on_output) = on_output.into_js_value().dyn_into::<Function>() else {
        unreachable!()
    };
    let Ok(on_error) = on_error.into_js_value().dyn_into::<Function>() else {
        unreachable!()
    };
    web_sys::VideoDecoder::new(&VideoDecoderInit::new(&on_error, &on_output))
        .map_err(|err| DecodingError::DecoderSetupFailure(js_error_to_string(&err)))
}

fn js_video_decoder_config(config: &re_video::Config) -> VideoDecoderConfig {
    let js = VideoDecoderConfig::new(&config.codec);
    js.set_coded_width(config.coded_width as u32);
    js.set_coded_height(config.coded_height as u32);
    let description = Uint8Array::new_with_length(config.description.len() as u32);
    description.copy_from(&config.description[..]);
    js.set_description(&description);
    js.set_optimize_for_latency(true);
    js
}

fn js_error_to_string(v: &wasm_bindgen::JsValue) -> String {
    if let Some(v) = v.as_string() {
        return v;
    }

    if let Some(v) = v.dyn_ref::<js_sys::Error>() {
        return std::string::ToString::to_string(&v.to_string());
    }

    format!("{v:#?}")
}
