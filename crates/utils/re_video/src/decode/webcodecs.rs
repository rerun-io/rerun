use std::collections::hash_map::Entry;
use std::sync::LazyLock;

use ahash::HashMap;
use crossbeam::channel::Sender;
use js_sys::{Function, Uint8Array};
use re_mp4::StsdBoxContent;
use smallvec::SmallVec;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::closure::Closure;
use web_sys::{
    EncodedVideoChunk, EncodedVideoChunkInit, EncodedVideoChunkType, VideoDecoderConfig,
    VideoDecoderInit,
};

use super::{AsyncDecoder, Chunk, DecodeHardwareAcceleration, Frame, FrameInfo, Result};
use crate::{
    DecodeError, FrameResult, Time, Timescale, VideoCodec, VideoDataDescription,
    VideoEncodingDetails,
};

#[derive(Clone)]
#[repr(transparent)]
pub struct WebVideoFrame(web_sys::VideoFrame);

impl re_byte_size::SizeBytes for WebVideoFrame {
    fn heap_size_bytes(&self) -> u64 {
        0 // Part of Browser's memory, not wasm heap.
    }
}

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

/// Messages sent to the output callback.
enum OutputCallbackMessage {
    Reset,
    FrameInfo {
        web_timestamp_us: u64,
        frame_info: FrameInfo,
    },
}

pub struct WebVideoDecoder {
    codec: VideoCodec,

    timescale: Timescale,
    first_frame_pts: Time,

    decoder: web_sys::VideoDecoder,
    hw_acceleration: DecodeHardwareAcceleration,
    output_sender: crossbeam::channel::Sender<FrameResult>,

    output_callback_tx: Sender<OutputCallbackMessage>,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum WebError {
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

    #[error("The WebCodec decoder shut down unexpectedly.")]
    UnexpectedShutdown,

    #[error(
        "Not enough codec information to configure the video decoder. For live streams this typically happens prior to the arrival of the first key frame."
    )]
    NotEnoughCodecInformation,
}

// SAFETY: There is no way to access the same JS object from different OS threads
//         in a way that could result in a data race.

#[expect(unsafe_code)]
#[expect(clippy::undocumented_unsafe_blocks)] // false positive
unsafe impl Send for WebVideoDecoder {}

#[expect(unsafe_code)]
#[expect(clippy::undocumented_unsafe_blocks)]
unsafe impl Sync for WebVideoDecoder {}

#[expect(unsafe_code)]
#[expect(clippy::undocumented_unsafe_blocks)]
unsafe impl Send for WebVideoFrame {}

#[expect(unsafe_code)]
#[expect(clippy::undocumented_unsafe_blocks)]
unsafe impl Sync for WebVideoFrame {}

static IS_SAFARI: LazyLock<bool> = LazyLock::new(|| {
    web_sys::window().is_some_and(|w| w.has_own_property(&wasm_bindgen::JsValue::from("safari")))
});

static IS_FIREFOX: LazyLock<bool> = LazyLock::new(|| {
    web_sys::window()
        .and_then(|w| w.navigator().user_agent().ok())
        .is_some_and(|ua| ua.to_lowercase().contains("firefox"))
});

impl Drop for WebVideoDecoder {
    fn drop(&mut self) {
        re_log::debug!("Dropping WebVideoDecoder");
        if *IS_FIREFOX {
            // As of Firefox 140.0.4 we observe frequent tab crashes when calling `close` on a video decoder.
            // It would be nice to at least call `reset` instead, but that _also_ tends to crash the tab.
            // See https://bugzilla.mozilla.org/show_bug.cgi?id=1976929 for more details.
            return;
        }

        if let Err(err) = self.decoder.close() {
            if let Some(dom_exception) = err.dyn_ref::<web_sys::DomException>()
                && dom_exception.code() == web_sys::DomException::INVALID_STATE_ERR
            {
                // Invalid state error after a decode error may happen, ignore it!
                // TODO(andreas): we used to do so only if there was a non-flushed error. Are we ignoring this too eagerly?
                return;
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
        video_descr: &VideoDataDescription,
        hw_acceleration: DecodeHardwareAcceleration,
        output_sender: crossbeam::channel::Sender<FrameResult>,
    ) -> Result<Self, WebError> {
        // Web APIs insist on microsecond timestamps throughout.
        // If we don't have a timescale, assume a 30fps video where time units are frames.
        // Higher fps should be still just fine, the web just needs _something_.
        // For details on how we treat timestamps, see submit_chunk.
        let timescale = video_descr.timescale.unwrap_or(Timescale::new(30));

        let (decoder, output_callback_tx) = init_video_decoder(output_sender.clone())?;

        let first_frame_pts = video_descr
            .samples
            .iter()
            .find_map(|s| s.sample())
            .map_or(Time::ZERO, |s| s.presentation_timestamp);

        Ok(Self {
            codec: video_descr.codec,

            timescale,
            first_frame_pts,

            decoder,
            hw_acceleration,
            output_sender,
            output_callback_tx,
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

        // WebCodec consumes timestamps in microseconds.
        // The exact role of the timestamps in the decoding process is not specified as far as I can tell,
        // but it's reasonable to assume they play a role in ordering of outputs and are therefore important
        // in the presence of H.264/H.265 bframes, where sample/chunk-data is not submitted in presentation order.
        // In fact, experimenting with frame numbers instead of timestamps worked fine just as well in the absence of bframes.
        // However, to be on the safe side we stick with meaningful timestamps in microseconds.
        //
        // It is specified that the resulting frames are associated with the exact timestamp of the chunk.
        // Therefore, we should be able to use the timestamp to match the frame to the frameinfo!
        // See https://www.w3.org/TR/webcodecs/#output-videoframes:
        // > Let timestamp and duration be the timestamp and duration from the EncodedVideoChunk associated with output.
        //
        // However, something to be careful about is that the timestamps are internally represented as i64.
        // So any floating point number we pass in will be truncated.
        // To hedge against precision issues with epoch-style (and similar) timestamps we also offset with the first frame timestamp.
        //
        // We use the resulting timestamp as a key for retrieving frame infos.
        // Note that we have to be robust against duplicated timeestamps:
        // these may occur in theory due to rounding to whole microseconds or simply because a user specified more than one frame for a given timestamp.
        let web_timestamp_us = (video_chunk.presentation_timestamp - self.first_frame_pts)
            .into_micros(self.timescale) as u64;

        if self
            .output_callback_tx
            .send(OutputCallbackMessage::FrameInfo {
                web_timestamp_us,
                frame_info: FrameInfo {
                    frame_nr: Some(video_chunk.frame_nr),
                    is_sync: Some(video_chunk.is_sync),
                    sample_idx: Some(video_chunk.sample_idx),
                    presentation_timestamp: video_chunk.presentation_timestamp,
                    duration: video_chunk.duration,
                    latest_decode_timestamp: Some(video_chunk.decode_timestamp),
                },
            })
            .is_err()
        {
            return Err(DecodeError::WebDecoder(WebError::UnexpectedShutdown));
        }

        // Setup chunk for the WebCodec decoder to decode.
        let web_chunk = EncodedVideoChunkInit::new(&data, web_timestamp_us as _, type_);

        // Empirically, decoders don't care about the presence of the duration field.
        // The spec also marks it as entirely optional, but does not specify whether it may affect the decoding itself.
        // Given that we err on the side of providing too much than too little information.
        if let Some(duration) = video_chunk.duration {
            let duration_micros = 1e-3 * duration.duration(self.timescale).as_nanos() as f64;
            web_chunk.set_duration(duration_micros);
        }

        let web_chunk = EncodedVideoChunk::new(&web_chunk)
            .map_err(|err| WebError::CreateChunk(js_error_to_string(&err)))?;
        self.decoder
            .decode(&web_chunk)
            .map_err(|err| WebError::DecodeChunk(js_error_to_string(&err)))?;

        Ok(())
    }

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self, video_descr: &VideoDataDescription) -> Result<()> {
        re_log::trace!("Resetting video decoder.");

        // Tell the output callback to discard all previous frame info.
        // If we don't do that, they may either linger indefinitely or be overwritten by new frames.
        self.output_callback_tx
            .send(OutputCallbackMessage::Reset)
            .ok();

        if *IS_FIREFOX {
            // As of Firefox 140.0.4 we observe frequent tab crashes when calling `reset` on a video decoder.
            // See https://bugzilla.mozilla.org/show_bug.cgi?id=1976929 for more details.
            let (decoder, output_callback_tx) = init_video_decoder(self.output_sender.clone())?;
            self.decoder = decoder;
            self.output_callback_tx = output_callback_tx;
        } else if let Err(_err) = self.decoder.reset() {
            // It can happen that reset fails after a previously encountered error.
            // In that case, start over completely and try again!
            re_log::debug!("Video decoder reset failed, recreating decoder.");
            let (decoder, output_callback_tx) = init_video_decoder(self.output_sender.clone())?;
            self.decoder = decoder;
            self.output_callback_tx = output_callback_tx;
        }

        // For all we know, the first frame timestamp may have changed.
        self.first_frame_pts = video_descr
            .samples
            .iter()
            .find_map(|s| s.sample())
            .map_or(Time::ZERO, |s| s.presentation_timestamp);

        let encoding_details = video_descr
            .encoding_details
            .as_ref()
            .ok_or(WebError::NotEnoughCodecInformation)?;

        self.decoder
            .configure(&js_video_decoder_config(
                encoding_details,
                self.hw_acceleration,
            ))
            .map_err(|err| WebError::ConfigureFailure(js_error_to_string(&err)).into())
    }

    /// Called after submitting the last chunk.
    ///
    /// Should flush all pending frames.
    fn end_of_video(&mut self) -> Result<()> {
        // This returns a promise that resolves once all pending messages have been processed.
        // https://developer.mozilla.org/en-US/docs/Web/API/VideoDecoder/flush
        //
        // It has been observed that if we don't call this, it can happen that the last few frames are never decoded.
        // Notably, MDN writes about flush methods in general here https://developer.mozilla.org/en-US/docs/Web/API/WebCodecs_API#processing_model
        // """
        // Methods named flush() can be used to wait for the completion of all work that was pending at the time flush() was called.
        // However, it should generally only be called once all desired work is queued.
        // It is not intended to force progress at regular intervals.
        // Calling it unnecessarily will affect encoder quality and cause decoders to require the next input to be a key frame.
        // """
        // -> Nothing of this indicates that we _have_ to call it and rather discourages it,
        // but it points out that it might be a good idea once "all desired work is queued".
        //
        // Note that the next frame after a flush has to be a key frame.
        // This is already part of the contract for `AsyncDecoder::end_of_video`.
        let flush_promise = self.decoder.flush();

        // If we don't handle potential flush errors, we'll get a lot of spam in the console.
        wasm_bindgen_futures::spawn_local(async move {
            let flush_result = wasm_bindgen_futures::JsFuture::from(flush_promise).await;
            if let Err(flush_error) = flush_result {
                if let Some(dom_exception) = flush_error.dyn_ref::<web_sys::DomException>()
                    && dom_exception.code() == web_sys::DomException::ABORT_ERR
                {
                    // Video decoder got closed, that's fine.
                    return;
                }

                re_log::debug!(
                    "Failed to flush video: {}",
                    js_error_to_string(&flush_error)
                );
            }
        });

        Ok(())
    }

    fn min_num_samples_to_enqueue_ahead(&self) -> usize {
        // TODO(#8848): For some h264 videos (which??) we need to enqueue more samples, otherwise Safari will not provide us with any frames.
        // (The same happens with FFmpeg-cli decoder for the affected videos)
        if self.codec == VideoCodec::H264 && *IS_SAFARI {
            16 // Safari needs more samples queued for h264
        } else {
            // No such workaround are needed anywhere else,
            // GOP boundaries as handled by the video player are enough.
            0
        }
    }
}

fn init_video_decoder(
    output_sender: crossbeam::channel::Sender<FrameResult>,
) -> Result<(web_sys::VideoDecoder, Sender<OutputCallbackMessage>), WebError> {
    let (output_callback_tx, output_callback_rx) = crossbeam::channel::unbounded();

    let on_output = {
        let output_sender = output_sender.clone();

        // Timestamps _should_ be unique.
        // But a user may supply multiple frame on the same timestamp or timestamps so close to each other that we can't distinguish them here
        // (since WebCodec timestamps are whole microseconds whereas our "native" timestamps are on the custom video timescale)
        // Either way, we have to handle this gracefully.
        //
        // WebCodec spec says that all frames should come in presentation order.
        // This means that `pending_frame_infos` should be able to be just a simple queue.
        // However, in practice we observed that Firefox & Safari don't always stick to that.
        // See https://github.com/rerun-io/rerun/pull/10405
        let mut pending_frame_infos: HashMap<u64, SmallVec<[FrameInfo; 1]>> = HashMap::default();

        // This closure has been observed to be called truly asynchronously on Firefox.
        // -> Do *NOT* use any locks in here, since parking lot isn't supported on web.
        Closure::wrap(Box::new(move |frame: web_sys::VideoFrame| {
            // First thing we wrap the frame to it gets closed on drop (i.e even if something goes wrong).
            let frame = WebVideoFrame(frame);

            loop {
                match output_callback_rx.try_recv() {
                    Ok(OutputCallbackMessage::FrameInfo {
                        web_timestamp_us,
                        frame_info,
                    }) => {
                        let infos = pending_frame_infos.entry(web_timestamp_us).or_default();
                        infos.push(frame_info);
                    }

                    Ok(OutputCallbackMessage::Reset) => {
                        pending_frame_infos.clear();
                    }

                    Err(crossbeam::channel::TryRecvError::Empty) => {
                        // Done, received all messages.
                        break;
                    }

                    Err(crossbeam::channel::TryRecvError::Disconnected) => {
                        // We're probably shutting down.
                        return;
                    }
                }
            }

            let Some(web_timestamp_us_raw) = frame.timestamp() else {
                // Spec says this should never happen.
                re_log::warn_once!("WebCodec decoded video frame without any timestamp data.");
                return;
            };
            // WebCodec timestamps are internally represented as i64 according to the spec.
            // Any floating point part would be a violation of the spec.
            let web_timestamp_us = web_timestamp_us_raw as u64;

            match pending_frame_infos.entry(web_timestamp_us) {
                Entry::Occupied(mut entry) => {
                    let infos = entry.get_mut();
                    if infos.is_empty() {
                        entry.remove();
                        re_log::error_once!(
                            "No more frame infos for timestamp {web_timestamp_us}. This is an implementation bug."
                        );
                        return;
                    }

                    // If there's several frame infos for the same timestamp,
                    // use the oldest one.
                    let info = infos.remove(0);
                    if infos.is_empty() {
                        entry.remove();
                    }

                    output_sender
                        .send(Ok(Frame {
                            content: frame,
                            info,
                        }))
                        .ok();
                }

                Entry::Vacant(_) => {
                    re_log::warn!(
                        "Decoder produced a frame at timestamp {web_timestamp_us_raw}us for which we don't have a valid frame info."
                    );
                }
            }
        }) as Box<dyn FnMut(web_sys::VideoFrame)>)
    };

    let on_error = Closure::wrap(Box::new(move |err: js_sys::Error| {
        output_sender
            .send(Err(super::DecodeError::WebDecoder(WebError::Decoding(
                js_error_to_string(&err),
            ))))
            .ok();
    }) as Box<dyn FnMut(js_sys::Error)>);

    let Ok(on_output) = on_output.into_js_value().dyn_into::<Function>() else {
        unreachable!()
    };
    let Ok(on_error) = on_error.into_js_value().dyn_into::<Function>() else {
        unreachable!()
    };

    let decoder = web_sys::VideoDecoder::new(&VideoDecoderInit::new(&on_error, &on_output))
        .map_err(|err| WebError::DecoderSetupFailure(js_error_to_string(&err)))?;

    Ok((decoder, output_callback_tx))
}

fn js_video_decoder_config(
    encoding_details: &VideoEncodingDetails,
    hw_acceleration: DecodeHardwareAcceleration,
) -> VideoDecoderConfig {
    let js = VideoDecoderConfig::new(&encoding_details.codec_string);
    js.set_coded_width(encoding_details.coded_dimensions[0] as u32);
    js.set_coded_height(encoding_details.coded_dimensions[1] as u32);

    if let Some(stsd) = &encoding_details.stsd {
        let description = match &stsd.contents {
            StsdBoxContent::Av01(content) => Some(content.av1c.raw.clone()),
            StsdBoxContent::Avc1(content) => Some(content.avcc.raw.clone()),
            StsdBoxContent::Hev1(content) | StsdBoxContent::Hvc1(content) => {
                Some(content.hvcc.raw.clone())
            }
            StsdBoxContent::Vp08(content) => Some(content.vpcc.raw.clone()),
            StsdBoxContent::Vp09(content) => Some(content.vpcc.raw.clone()),
            StsdBoxContent::Mp4a(_) | StsdBoxContent::Tx3g(_) | StsdBoxContent::Unknown(_) => {
                if cfg!(debug_assertions) {
                    unreachable!("Unknown codec should be caught earlier.")
                }
                None
            }
        };

        if let Some(description_raw) = description {
            let description = Uint8Array::new_with_length(description_raw.len() as u32);
            description.copy_from(&description_raw[..]);
            js.set_description(&description);
        }
    } else {
        // For H264 & H265, the bitstream is assumed to be in Annex B format if no AVCC box is present.
        // * H264: https://www.w3.org/TR/webcodecs-avc-codec-registration/#videodecoderconfig-description
        // * H265: https://www.w3.org/TR/webcodecs-hevc-codec-registration/#videodecoderconfig-description
    }

    // "If true this is a hint that the selected decoder should be optimized to minimize the number
    // of EncodedVideoChunk objects that have to be decoded before a VideoFrame is output."
    // https://developer.mozilla.org/en-US/docs/Web/API/VideoDecoder/configure#optimizeforlatency
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
            .map_or_else(|| error.clone(), |s| s.to_owned());
    }

    format!("{v:#?}")
}
