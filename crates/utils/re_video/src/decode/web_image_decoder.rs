use js_sys::Uint8Array;
use wasm_bindgen::JsCast as _;

use super::{AsyncDecoder, Chunk, Frame, FrameInfo, Result};
use crate::{DecodeError, FrameResult, Sender, VideoDataDescription};

pub struct WebImageDecoder {
    image_mime_type: String,
    output_sender: Sender<FrameResult>,
}

impl WebImageDecoder {
    pub fn try_new(
        video_descr: &VideoDataDescription,
        output_sender: Sender<FrameResult>,
    ) -> Option<Self> {
        Some(Self {
            image_mime_type: video_descr.image_codec_mime_type()?.to_owned(),
            output_sender,
        })
    }
}

impl AsyncDecoder for WebImageDecoder {
    fn submit_chunk(&mut self, chunk: Chunk) -> Result<()> {
        let output_sender = self.output_sender.clone();
        let mime_type = self.image_mime_type.clone();

        wasm_bindgen_futures::spawn_local(async move {
            match decode_image(chunk, &mime_type).await {
                Ok(frame) => {
                    output_sender.send(Ok(frame)).ok();
                }
                Err(err) => {
                    output_sender.send(Err(err)).ok();
                }
            }
        });

        Ok(())
    }

    fn reset(&mut self, descr: &VideoDataDescription) -> Result<()> {
        if let Some(encoding_details) = &descr.encoding_details {
            self.image_mime_type = encoding_details.codec_string.clone();
        }
        Ok(())
    }
}

async fn decode_image(chunk: Chunk, mime_type: &str) -> Result<Frame> {
    // Create a Blob from the image data.
    let uint8_array = Uint8Array::from(chunk.data.as_slice());
    let parts = js_sys::Array::new();
    parts.push(&uint8_array);

    let options = web_sys::BlobPropertyBag::new();
    if !mime_type.is_empty() {
        options.set_type(mime_type);
    }

    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(
        |js_err| {
            DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(format!(
                "Failed to create Blob: {js_err:?}"
            )))
        },
    )?;

    // Decode the image using createImageBitmap.
    let window = web_sys::window().ok_or_else(|| {
        DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(
            "No global window object".to_owned(),
        ))
    })?;

    let promise = window
        .create_image_bitmap_with_blob(&blob)
        .map_err(|js_err| {
            DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(format!(
                "createImageBitmap failed: {js_err:?}"
            )))
        })?;

    let bitmap_js = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|js_err| {
            DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(format!(
                "createImageBitmap rejected: {js_err:?}"
            )))
        })?;

    let bitmap: web_sys::ImageBitmap = bitmap_js.dyn_into().map_err(|_js_err| {
        DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(
            "createImageBitmap did not return an ImageBitmap".to_owned(),
        ))
    })?;

    // Create a VideoFrame from the ImageBitmap.
    // The timestamp is required by the VideoFrame constructor.
    let init = web_sys::VideoFrameInit::new();
    init.set_timestamp(0.0);
    let video_frame = web_sys::VideoFrame::new_with_image_bitmap_and_video_frame_init(
        &bitmap, &init,
    )
    .map_err(|js_err| {
        DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(format!(
            "Failed to create VideoFrame from ImageBitmap: {js_err:?}"
        )))
    })?;

    Ok(Frame {
        content: super::webcodecs::WebVideoFrame(video_frame),
        info: FrameInfo {
            is_sync: Some(true),
            sample_idx: Some(chunk.sample_idx),
            frame_nr: Some(chunk.frame_nr),
            presentation_timestamp: chunk.presentation_timestamp,
            duration: chunk.duration,
            latest_decode_timestamp: Some(chunk.decode_timestamp),
        },
    })
}
