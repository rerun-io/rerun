use std::sync::Arc;

use js_sys::{Function, Uint8Array};
use parking_lot::Mutex;
use wasm_bindgen::{closure::Closure, JsCast as _};
use web_sys::{
    EncodedVideoChunk, EncodedVideoChunkInit, EncodedVideoChunkType, VideoDecoderConfig,
    VideoDecoderInit,
};

use re_video::{Time, Timescale};

use crate::{
    video::{DecodeHardwareAcceleration, DecodingError},
    RenderContext,
};

use super::{latest_at_idx, TimedDecodingError, VideoChunkDecoder, VideoTexture};

#[derive(Clone)]
#[repr(transparent)]
struct WebVideoFrame(web_sys::VideoFrame);

impl Drop for WebVideoFrame {
    fn drop(&mut self) {
        self.0.close();
    }
}

impl std::ops::Deref for WebVideoFrame {
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

    inner: WebVideoFrame,
}

struct DecoderOutput {
    frames: Vec<BufferedFrame>,

    /// Set on error; reset on success.
    error: Option<TimedDecodingError>,
}

pub struct WebVideoDecoder {
    data: Arc<re_video::VideoData>,
    decoder: web_sys::VideoDecoder,
    decoder_output: Arc<Mutex<DecoderOutput>>,
    hw_acceleration: DecodeHardwareAcceleration,
}

// SAFETY: There is no way to access the same JS object from different OS threads
//         in a way that could result in a data race.

#[allow(unsafe_code)]
// Clippy did not recognize a safety comment on these impls no matter what I tried:
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe impl Send for WebVideoDecoder {}

#[allow(unsafe_code)]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe impl Sync for WebVideoDecoder {}

#[allow(unsafe_code)]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe impl Send for WebVideoFrame {}

#[allow(unsafe_code)]
#[allow(clippy::undocumented_unsafe_blocks)]
unsafe impl Sync for WebVideoFrame {}

impl Drop for WebVideoDecoder {
    fn drop(&mut self) {
        re_log::debug!("Dropping WebVideoDecoder");
        if let Err(err) = self.decoder.close() {
            if let Some(dom_exception) = err.dyn_ref::<web_sys::DomException>() {
                if dom_exception.code() == web_sys::DomException::INVALID_STATE_ERR
                    && self.decoder_output.lock().error.is_some()
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

impl WebVideoDecoder {
    pub fn new(
        data: Arc<re_video::VideoData>,
        hw_acceleration: DecodeHardwareAcceleration,
    ) -> Result<Self, DecodingError> {
        let decoder_output = Arc::new(Mutex::new(DecoderOutput {
            frames: Vec::new(),
            error: None,
        }));
        let decoder = init_video_decoder(&decoder_output, data.timescale)?;

        Ok(Self {
            data,
            decoder,
            decoder_output,
            hw_acceleration,
        })
    }
}

impl VideoChunkDecoder for WebVideoDecoder {
    /// Start decoding the given chunk.
    fn decode(
        &mut self,
        video_chunk: re_video::Chunk,
        is_keyframe: bool,
    ) -> Result<(), DecodingError> {
        let data = Uint8Array::from(video_chunk.data.as_slice());
        let type_ = if is_keyframe {
            EncodedVideoChunkType::Key
        } else {
            EncodedVideoChunkType::Delta
        };
        let web_chunk = EncodedVideoChunkInit::new(
            &data,
            video_chunk.timestamp.into_micros(self.data.timescale),
            type_,
        );
        web_chunk.set_duration(video_chunk.duration.into_micros(self.data.timescale));
        let web_chunk = EncodedVideoChunk::new(&web_chunk)
            .map_err(|err| DecodingError::CreateChunk(js_error_to_string(&err)))?;
        self.decoder
            .decode(&web_chunk)
            .map_err(|err| DecodingError::DecodeChunk(js_error_to_string(&err)))
    }

    fn update_video_texture(
        &mut self,
        render_ctx: &RenderContext,
        video_texture: &mut VideoTexture,
        presentation_timestamp: Time,
    ) -> Result<(), DecodingError> {
        let mut decoder_output = self.decoder_output.lock();

        let frames = &mut decoder_output.frames;

        let Some(frame_idx) = latest_at_idx(
            frames,
            |frame| frame.composition_timestamp,
            &presentation_timestamp,
        ) else {
            return Err(DecodingError::EmptyBuffer);
        };

        // drain up-to (but not including) the frame idx, clearing out any frames
        // before it. this lets the video decoder output more frames.
        drop(frames.drain(0..frame_idx));

        // after draining all old frames, the next frame will be at index 0
        let frame_idx = 0;
        let frame = &frames[frame_idx];

        let frame_time_range =
            frame.composition_timestamp..frame.composition_timestamp + frame.duration;

        if frame_time_range.contains(&presentation_timestamp)
            && video_texture.time_range != frame_time_range
        {
            copy_video_frame_to_texture(
                &render_ctx.queue,
                &frame.inner,
                &video_texture.texture.texture,
            );
            video_texture.time_range = frame_time_range;
        }

        Ok(())
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
            decoder_output.error = None;
            decoder_output.frames.clear();
        }

        Ok(())
    }

    /// Return and clear the latest error that happened during decoding.
    fn take_error(&mut self) -> Option<TimedDecodingError> {
        self.decoder_output.lock().error.take()
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
            let frame = WebVideoFrame(frame);

            let mut output = decoder_output.lock();
            output.frames.push(BufferedFrame {
                composition_timestamp,
                duration,
                inner: frame,
            });

            output.error = None; // We successfully decoded a frame, reset the error state.
        }) as Box<dyn Fn(web_sys::VideoFrame)>)
    };

    let on_error = {
        let decoder_output = decoder_output.clone();
        Closure::wrap(Box::new(move |err: js_sys::Error| {
            let err = DecodingError::Decoding(js_error_to_string(&err));

            let mut output = decoder_output.lock();
            if let Some(error) = &mut output.error {
                error.latest_error = err;
            } else {
                output.error = Some(TimedDecodingError::new(err));
            }
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
    let js = VideoDecoderConfig::new(&config.stsd.contents.codec_string().unwrap_or_default());
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
