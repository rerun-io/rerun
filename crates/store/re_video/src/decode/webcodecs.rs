use std::sync::Arc;

use js_sys::{Function, Uint8Array};
use wasm_bindgen::{closure::Closure, JsCast as _};
use web_sys::{
    EncodedVideoChunk, EncodedVideoChunkInit, EncodedVideoChunkType, VideoDecoderConfig,
    VideoDecoderInit,
};

use super::{
    AsyncDecoder, Chunk, DecodeHardwareAcceleration, Frame, FrameInfo, OutputCallback, Result,
};
use crate::{Config, Time, Timescale, VideoData};

#[derive(Clone)]
#[repr(transparent)]
pub struct WebVideoFrame(web_sys::VideoFrame);

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

pub struct WebVideoDecoder {
    video_config: Config,
    timescale: Timescale,
    minimum_presentation_timestamp: Time,
    decoder: web_sys::VideoDecoder,
    hw_acceleration: DecodeHardwareAcceleration,
    on_output: Arc<OutputCallback>,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("Failed to create VideoDecoder: {0}")]
    DecoderSetupFailure(String),

    #[error("Failed to configure the video decoder: {0}")]
    ConfigureFailure(String),

    /// e.g. unsupported codec
    #[error("Failed to create video chunk: {0}")]
    CreateChunk(String),

    /// e.g. unsupported codec
    #[error("Failed to decode video chunk: {0}")]
    DecodeChunk(String),

    /// e.g. unsupported codec
    #[error("Failed to decode video: {0}")]
    Decoding(String),
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
                if dom_exception.code() == web_sys::DomException::INVALID_STATE_ERR {
                    // Invalid state error after a decode error may happen, ignore it!
                    // TODO(andreas): we used to do so only if there was a non-flushed error. Are we ignoring this too eagerly?
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
        video: &VideoData,
        hw_acceleration: DecodeHardwareAcceleration,
        on_output: impl Fn(Result<Frame>) + Send + Sync + 'static,
    ) -> Result<Self, Error> {
        let on_output = Arc::new(on_output);
        let decoder = init_video_decoder(
            on_output.clone(),
            video.timescale,
            video.samples_statistics.minimum_presentation_timestamp,
        )?;

        Ok(Self {
            video_config: video.config.clone(),
            timescale: video.timescale,
            minimum_presentation_timestamp: video.samples_statistics.minimum_presentation_timestamp,
            decoder,
            hw_acceleration,
            on_output,
        })
    }
}

impl AsyncDecoder for WebVideoDecoder {
    /// Start decoding the given chunk.
    fn submit_chunk(&mut self, video_chunk: Chunk) -> Result<()> {
        let data = Uint8Array::from(video_chunk.data.as_slice());
        let type_ = if video_chunk.is_sync {
            EncodedVideoChunkType::Key
        } else {
            EncodedVideoChunkType::Delta
        };
        let web_chunk = EncodedVideoChunkInit::new(
            &data,
            video_chunk
                .presentation_timestamp
                .into_micros_since_start(self.timescale, self.minimum_presentation_timestamp),
            type_,
        );

        let duration_millis =
            1e-3 * video_chunk.duration.duration(self.timescale).as_nanos() as f64;
        web_chunk.set_duration(duration_millis);
        let web_chunk = EncodedVideoChunk::new(&web_chunk)
            .map_err(|err| Error::CreateChunk(js_error_to_string(&err)))?;
        self.decoder
            .decode(&web_chunk)
            .map_err(|err| Error::DecodeChunk(js_error_to_string(&err)))?;

        Ok(())
    }

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self) -> Result<()> {
        re_log::trace!("Resetting video decoder.");

        if let Err(_err) = self.decoder.reset() {
            // At least on Firefox, it can happen that reset on a previous error fails.
            // In that case, start over completely and try again!
            re_log::debug!("Video decoder reset failed, recreating decoder.");
            self.decoder = init_video_decoder(
                self.on_output.clone(),
                self.timescale,
                self.minimum_presentation_timestamp,
            )?;
        };

        self.decoder
            .configure(&js_video_decoder_config(
                &self.video_config,
                self.hw_acceleration,
            ))
            .map_err(|err| Error::ConfigureFailure(js_error_to_string(&err)))?;

        Ok(())
    }
}

fn init_video_decoder(
    on_output_callback: Arc<OutputCallback>,
    timescale: Timescale,
    minimum_presentation_timestamp: Time,
) -> Result<web_sys::VideoDecoder, Error> {
    let on_output = {
        let on_output = on_output_callback.clone();
        Closure::wrap(Box::new(move |frame: web_sys::VideoFrame| {
            // We assume that the timestamp returned by the decoder is in time since start,
            // and does not represent demuxed "raw" presentation timestamps.
            let presentation_timestamp = Time::from_micros_since_start(
                frame.timestamp().unwrap_or(0.0),
                timescale,
                minimum_presentation_timestamp,
            );
            let duration = Time::from_micros_since_start(
                frame.duration().unwrap_or(0.0),
                timescale,
                minimum_presentation_timestamp,
            );

            on_output(Ok(Frame {
                content: WebVideoFrame(frame),
                info: FrameInfo {
                    presentation_timestamp,
                    duration,
                    ..Default::default()
                },
            }));
        }) as Box<dyn Fn(web_sys::VideoFrame)>)
    };

    let on_error = Closure::wrap(Box::new(move |err: js_sys::Error| {
        on_output_callback(Err(super::Error::WebDecoder(Error::Decoding(
            js_error_to_string(&err),
        ))));
    }) as Box<dyn Fn(js_sys::Error)>);

    let Ok(on_output) = on_output.into_js_value().dyn_into::<Function>() else {
        unreachable!()
    };
    let Ok(on_error) = on_error.into_js_value().dyn_into::<Function>() else {
        unreachable!()
    };

    web_sys::VideoDecoder::new(&VideoDecoderInit::new(&on_error, &on_output))
        .map_err(|err| Error::DecoderSetupFailure(js_error_to_string(&err)))
}

fn js_video_decoder_config(
    config: &Config,
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
        // Firefox prefixes most decoding errors with "EncodingError: ", which isn't super helpful.
        let error = std::string::ToString::to_string(&v.to_string());
        return error
            .strip_prefix("EncodingError: ")
            .map_or(error.clone(), |s| s.to_owned());
    }

    format!("{v:#?}")
}
