use std::{sync::Arc, time::Duration};

use js_sys::{Function, Uint8Array};
use parking_lot::Mutex;
use re_video::{Time, Timescale};
use wasm_bindgen::{closure::Closure, JsCast as _};
use web_sys::{
    EncodedVideoChunk, EncodedVideoChunkInit, EncodedVideoChunkType, VideoDecoderConfig,
    VideoDecoderInit,
};
use web_time::Instant;

use super::latest_at_idx;
use crate::{
    resource_managers::GpuTexture2D,
    video::{DecodeHardwareAcceleration, DecodingError, FrameDecodingResult, VideoFrameTexture},
    DebugLabel, RenderContext,
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

struct DecoderOutput {
    frames: Vec<BufferedFrame>,

    last_decoding_error: Option<DecodingError>,

    /// Whether we reset the decoder since the last time an error was reported.
    reset_since_last_reported_error: bool,

    /// Time at which point `last_decoding_error` changed from `None` to `Some`.
    time_when_entering_error_state: Instant,
}

/// Delaying error reports (and showing last-good images meanwhile) allows us to skip over
/// transient errors without flickering.
const DECODING_ERROR_REPORTING_DELAY: Duration = Duration::from_millis(400);

pub struct VideoDecoder {
    data: Arc<re_video::VideoData>,
    queue: Arc<wgpu::Queue>,
    texture: GpuTexture2D,

    decoder: web_sys::VideoDecoder,
    decoder_output: Arc<Mutex<DecoderOutput>>,
    hw_acceleration: DecodeHardwareAcceleration,

    last_used_frame_timestamp: Time,
    current_segment_idx: usize,
    current_sample_idx: usize,

    error_on_last_frame_at: bool,
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
        re_log::debug!("Dropping VideoDecoder");
        if let Err(err) = self.decoder.close() {
            if let Some(dom_exception) = err.dyn_ref::<web_sys::DomException>() {
                if dom_exception.code() == web_sys::DomException::INVALID_STATE_ERR
                    && self.decoder_output.lock().last_decoding_error.is_some()
                {
                    // Invalid state error after a decode error may happen, ignore it!
                    return;
                }
            }

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
        hw_acceleration: DecodeHardwareAcceleration,
    ) -> Result<Self, DecodingError> {
        let decoder_output = Arc::new(Mutex::new(DecoderOutput {
            frames: Vec::new(),
            last_decoding_error: None,
            reset_since_last_reported_error: false,
            time_when_entering_error_state: Instant::now(),
        }));
        let decoder = init_video_decoder(&decoder_output, data.timescale)?;
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
            decoder_output,
            hw_acceleration,

            last_used_frame_timestamp: Time::MAX,
            current_segment_idx: usize::MAX,
            current_sample_idx: usize::MAX,

            error_on_last_frame_at: false,
        })
    }

    pub fn frame_at(
        &mut self,
        render_ctx: &RenderContext,
        presentation_timestamp_s: f64,
    ) -> FrameDecodingResult {
        let result = self.frame_at_internal(presentation_timestamp_s);
        match &result {
            Ok(VideoFrameTexture::Ready(_)) => {
                self.error_on_last_frame_at = false;
            }
            Ok(VideoFrameTexture::Pending(_)) => {
                if self.error_on_last_frame_at {
                    // If we switched from error to pending, clear the texture.
                    // This is important to avoid flickering, in particular when switching from
                    // benign errors like DecodingError::NegativeTimestamp.
                    // If we don't do this, we see the last valid texture which can look really weird.
                    self.clear_video_texture(render_ctx);
                }

                self.error_on_last_frame_at = false;
            }
            Err(_) => {
                self.error_on_last_frame_at = true;
            }
        }
        result
    }

    fn frame_at_internal(&mut self, presentation_timestamp_s: f64) -> FrameDecodingResult {
        if presentation_timestamp_s < 0.0 {
            return Err(DecodingError::NegativeTimestamp);
        }
        let presentation_timestamp = Time::from_secs(presentation_timestamp_s, self.data.timescale);
        self.enqueue_requested_segments(presentation_timestamp)?;
        self.try_present_frame(presentation_timestamp)
    }

    fn enqueue_requested_segments(
        &mut self,
        presentation_timestamp: Time,
    ) -> Result<(), DecodingError> {
        re_tracing::profile_function!();

        // Some terminology:
        //   - presentation timestamp = composition timestamp
        //     = the time at which the frame should be shown
        //   - decode timestamp
        //     = determines the decoding order of samples
        //
        // Note: `composition >= decode` for any given sample.
        //       For some codecs, the two timestamps are the same.
        // We must enqueue samples in decode order, but show them in composition order.

        // 1. Find the latest sample where `decode_timestamp <= presentation_timestamp`.
        //    Because `composition >= decode`, we never have to look further ahead in the
        //    video than this.
        let Some(decode_sample_idx) = latest_at_idx(
            &self.data.samples,
            |sample| sample.decode_timestamp,
            &presentation_timestamp,
        ) else {
            return Err(DecodingError::EmptyVideo);
        };

        // 2. Search _backwards_, starting at `decode_sample_idx`, looking for
        //    the first sample where `sample.composition_timestamp <= presentation_timestamp`.
        //    This is the sample which when decoded will be presented at the timestamp the user requested.
        let Some(requested_sample_idx) = self.data.samples[..=decode_sample_idx]
            .iter()
            .rposition(|sample| sample.composition_timestamp <= presentation_timestamp)
        else {
            return Err(DecodingError::EmptyVideo);
        };

        // 3. Do a binary search through segments by the decode timestamp of the found sample
        //    to find the segment that contains the sample.
        let Some(requested_segment_idx) = latest_at_idx(
            &self.data.segments,
            |segment| segment.start,
            &self.data.samples[requested_sample_idx].decode_timestamp,
        ) else {
            return Err(DecodingError::EmptyVideo);
        };

        // 4. Enqueue segments as needed.
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
        // We maintain a buffer of 2 segments, so we can always smoothly transition to the next segment.
        // We can always start decoding from any segment, because segments always begin with a keyframe.
        //
        // Backward seeks or seeks across many segments trigger a reset of the decoder,
        // because decoding all the samples between the previous sample and the requested
        // one would mean decoding and immediately discarding more frames than we need.
        if requested_segment_idx != self.current_segment_idx {
            let segment_distance = requested_segment_idx.checked_sub(self.current_segment_idx);
            if segment_distance == Some(1) {
                // forward seek to next segment - queue up the one _after_ requested
                self.enqueue_segment(requested_segment_idx + 1)?;
            } else {
                // Startup, forward seek by N>1, or backward seek across segments -> reset decoder
                self.reset()?;
                self.enqueue_segment(requested_segment_idx)?;
                self.enqueue_segment(requested_segment_idx + 1)?;
            }
        } else if requested_sample_idx != self.current_sample_idx {
            // special case: handle seeking backwards within a single segment
            // this is super inefficient, but it's the only way to handle it
            // while maintaining a buffer of 2 segments
            let sample_distance = requested_sample_idx as isize - self.current_sample_idx as isize;
            if sample_distance < 0 {
                self.reset()?;
                self.enqueue_segment(requested_segment_idx)?;
                self.enqueue_segment(requested_segment_idx + 1)?;
            }
        }

        // At this point, we have the requested segments enqueued. They will be output
        // in _composition timestamp_ order, so presenting the frame is a binary search
        // through the frame buffer as usual.

        self.current_segment_idx = requested_segment_idx;
        self.current_sample_idx = requested_sample_idx;

        Ok(())
    }

    fn try_present_frame(&mut self, presentation_timestamp: Time) -> FrameDecodingResult {
        re_tracing::profile_function!();

        let mut decoder_output = self.decoder_output.lock();

        let timescale = self.data.timescale;

        let frames = &mut decoder_output.frames;

        let Some(frame_idx) = latest_at_idx(
            frames,
            |frame| frame.composition_timestamp,
            &presentation_timestamp,
        ) else {
            // No buffered frames - texture will be blank.

            // Might this be due to an error?
            //
            // We only care about decoding errors when we don't find the requested frame,
            // since we want to keep playing the video fine even if parts of it are broken.
            // That said, practically we reset the decoder and thus all frames upon error,
            // so it doesn't make a lot of difference.
            if let Some(last_decoding_error) = decoder_output.last_decoding_error.clone() {
                if decoder_output.time_when_entering_error_state.elapsed()
                    >= DECODING_ERROR_REPORTING_DELAY
                {
                    // Report the error only if we have been in an error state for a certain amount of time.
                    // Don't immediately report the error, since we might immediately recover from it.
                    // Otherwise, this would cause aggressive flickering!
                    return Err(last_decoding_error);
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

        let frame_timestamp_ms = frame.composition_timestamp.into_millis(timescale);
        let frame_duration_ms = frame.duration.into_millis(timescale);

        // This handles the case when we have a buffered frame that's older than the requested timestamp.
        // We don't want to show this frame to the user, because it's not actually the one they requested,
        // so instead return the last decoded frame.
        if presentation_timestamp.into_millis(timescale) - frame_timestamp_ms > frame_duration_ms {
            return Ok(VideoFrameTexture::Pending(self.texture.clone()));
        }

        if self.last_used_frame_timestamp != frame.composition_timestamp {
            self.last_used_frame_timestamp = frame.composition_timestamp;
            copy_video_frame_to_texture(&self.queue, &frame.inner, &self.texture.texture);
        }

        Ok(VideoFrameTexture::Ready(self.texture.clone()))
    }

    /// Clears the texture that is shown on pending to black.
    fn clear_video_texture(&self, render_ctx: &RenderContext) {
        // Clear texture is a native only feature, so let's not do that.
        // before_view_builder_encoder.clear_texture(texture, subresource_range);

        // But our target is also a render target, so just create a dummy renderpass with clear.
        let mut before_view_builder_encoder =
            render_ctx.active_frame.before_view_builder_encoder.lock();
        let _ = before_view_builder_encoder
            .get()
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: DebugLabel::from("clear_video_texture").get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.texture.default_view,
                    resolve_target: None,
                    ops: wgpu::Operations::<wgpu::Color> {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
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
        let data = Uint8Array::from(
            &self.data.data[sample.byte_offset as usize
                ..sample.byte_offset as usize + sample.byte_length as usize],
        );
        let type_ = if is_key {
            EncodedVideoChunkType::Key
        } else {
            EncodedVideoChunkType::Delta
        };
        let chunk = EncodedVideoChunkInit::new(
            &data,
            sample
                .composition_timestamp
                .into_micros(self.data.timescale),
            type_,
        );
        chunk.set_duration(sample.duration.into_micros(self.data.timescale));
        let chunk = EncodedVideoChunk::new(&chunk)
            .map_err(|err| DecodingError::CreateChunk(js_error_to_string(&err)))?;
        self.decoder
            .decode(&chunk)
            .map_err(|err| DecodingError::DecodeChunk(js_error_to_string(&err)))
    }

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self) -> Result<(), DecodingError> {
        re_log::trace!("Resetting video decoder.");
        if let Err(_err) = self.decoder.reset() {
            // At least on Firefox, it can happen that reset on a previous error fails.
            // In that case, start over completely and try again!
            re_log::debug!("Video decoder reset failed, recreating decoder.");
            self.decoder = init_video_decoder(&self.decoder_output, self.data.timescale)?;
        };

        self.decoder
            .configure(&js_video_decoder_config(
                &self.data.config,
                self.hw_acceleration,
            ))
            .map_err(|err| DecodingError::ConfigureFailure(js_error_to_string(&err)))?;

        {
            let mut decoder_output = self.decoder_output.lock();
            decoder_output.reset_since_last_reported_error = true;
            drop(decoder_output.frames.drain(..));
        }

        self.current_segment_idx = usize::MAX;
        self.current_sample_idx = usize::MAX;

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
    decoder_output: &Arc<Mutex<DecoderOutput>>,
    timescale: Timescale,
) -> Result<web_sys::VideoDecoder, DecodingError> {
    let on_output = {
        let decoder_output = decoder_output.clone();
        Closure::wrap(Box::new(move |frame: web_sys::VideoFrame| {
            let composition_timestamp =
                Time::from_micros(frame.timestamp().unwrap_or(0.0), timescale);
            let duration = Time::from_micros(frame.duration().unwrap_or(0.0), timescale);
            let frame = VideoFrame(frame);

            let mut output = decoder_output.lock();
            output.frames.push(BufferedFrame {
                composition_timestamp,
                duration,
                inner: frame,
            });

            // We successfully decoded a frame, reset the error state.
            output.last_decoding_error = None;
        }) as Box<dyn Fn(web_sys::VideoFrame)>)
    };

    let on_error = {
        let decoder_output = decoder_output.clone();
        Closure::wrap(Box::new(move |err: js_sys::Error| {
            let err = js_error_to_string(&err);

            let mut output = decoder_output.lock();
            if output.last_decoding_error.is_none() {
                output.time_when_entering_error_state = Instant::now();
            }
            output.last_decoding_error = Some(DecodingError::Decoding(err));
            output.reset_since_last_reported_error = false;
        }) as Box<dyn Fn(js_sys::Error)>)
    };

    let Ok(on_output) = on_output.into_js_value().dyn_into::<Function>() else {
        unreachable!()
    };
    let Ok(on_error) = on_error.into_js_value().dyn_into::<Function>() else {
        unreachable!()
    };
    web_sys::VideoDecoder::new(&VideoDecoderInit::new(&on_error, &on_output))
        .map_err(|err| DecodingError::DecoderSetupFailure(js_error_to_string(&err)))
}

fn js_video_decoder_config(
    config: &re_video::Config,
    hw_acceleration: DecodeHardwareAcceleration,
) -> VideoDecoderConfig {
    let js = VideoDecoderConfig::new(&config.codec);
    js.set_coded_width(config.coded_width as u32);
    js.set_coded_height(config.coded_height as u32);
    let description = Uint8Array::new_with_length(config.description.len() as u32);
    description.copy_from(&config.description[..]);
    js.set_description(&description);
    js.set_optimize_for_latency(true);

    match hw_acceleration {
        DecodeHardwareAcceleration::Auto => {
            js.set_hardware_acceleration(web_sys::HardwareAcceleration::NoPreference);
        }
        DecodeHardwareAcceleration::PreferSoftware => {
            js.set_hardware_acceleration(web_sys::HardwareAcceleration::PreferSoftware);
        }
        DecodeHardwareAcceleration::PreferHardware => {
            js.set_hardware_acceleration(web_sys::HardwareAcceleration::PreferHardware);
        }
    }

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
