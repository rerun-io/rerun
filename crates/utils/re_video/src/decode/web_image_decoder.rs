use js_sys::Uint8Array;
use wasm_bindgen::JsCast as _;

use super::{
    AsyncDecoder, Chunk, Frame, FrameContent, FrameInfo, Result, webcodecs::string_from_js_value,
};
use crate::{ChromaSubsamplingModes, DecodeError, FrameResult, Sender, VideoDataDescription};

pub struct WebImageDecoder {
    image_mime_type: String,
    bit_depth: Option<u8>,
    chroma_subsampling: Option<ChromaSubsamplingModes>,
    output_sender: Sender<FrameResult>,
}

impl WebImageDecoder {
    pub fn try_new(
        video_descr: &VideoDataDescription,
        output_sender: Sender<FrameResult>,
    ) -> Option<Self> {
        Some(Self {
            image_mime_type: video_descr.image_codec_mime_type()?.to_owned(),
            bit_depth: video_descr
                .encoding_details
                .as_ref()
                .and_then(|details| details.bit_depth),
            chroma_subsampling: video_descr
                .encoding_details
                .as_ref()
                .and_then(|details| details.chroma_subsampling),
            output_sender,
        })
    }
}

impl AsyncDecoder for WebImageDecoder {
    fn submit_chunk(&mut self, chunk: Chunk) -> Result<()> {
        let output_sender = self.output_sender.clone();
        let mime_type = self.image_mime_type.clone();
        let bit_depth = self.bit_depth;
        let chroma_subsampling = self.chroma_subsampling;

        wasm_bindgen_futures::spawn_local(async move {
            match decode_image(chunk, &mime_type, bit_depth, chroma_subsampling).await {
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
            self.bit_depth = encoding_details.bit_depth;
            self.chroma_subsampling = encoding_details.chroma_subsampling;
        }
        Ok(())
    }
}

async fn decode_image(
    chunk: Chunk,
    mime_type: &str,
    bit_depth: Option<u8>,
    chroma_subsampling: Option<ChromaSubsamplingModes>,
) -> Result<Frame> {
    if should_decode_on_cpu(bit_depth, chroma_subsampling) {
        decode_image_on_cpu(chunk, mime_type)
    } else {
        match decode_image_with_browser(&chunk, mime_type).await {
            Ok(frame) => Ok(frame),
            Err(_) => decode_image_on_cpu(chunk, mime_type),
        }
    }
}

fn frame_info(chunk: &Chunk) -> FrameInfo {
    FrameInfo {
        is_sync: Some(true),
        sample_idx: Some(chunk.sample_idx),
        frame_nr: Some(chunk.frame_nr),
        presentation_timestamp: chunk.presentation_timestamp,
        duration: chunk.duration,
        latest_decode_timestamp: Some(chunk.decode_timestamp),
    }
}

async fn decode_image_with_browser(chunk: &Chunk, mime_type: &str) -> Result<Frame> {
    // Create a Blob from the image data.
    let uint8_array = Uint8Array::from(chunk.data.as_slice());
    let parts = js_sys::Array::new();
    parts.push(&uint8_array);

    let options = web_sys::BlobPropertyBag::new();
    if !mime_type.is_empty() {
        options.set_type(mime_type);
    }

    let blob =
        web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(|err| {
            DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(format!(
                "Failed to create Blob: {}",
                string_from_js_value(&err)
            )))
        })?;

    // Decode the image using createImageBitmap.
    let window = web_sys::window().ok_or_else(|| {
        DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(
            "No global window object".to_owned(),
        ))
    })?;

    let promise = window.create_image_bitmap_with_blob(&blob).map_err(|err| {
        DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(format!(
            "createImageBitmap failed: {}",
            string_from_js_value(&err)
        )))
    })?;

    let bitmap_js = promise.await.map_err(|err| {
        DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(format!(
            "createImageBitmap rejected: {}",
            string_from_js_value(&err)
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
    init.set_timestamp(0);
    let video_frame = web_sys::VideoFrame::new_with_image_bitmap_and_video_frame_init(
        &bitmap, &init,
    )
    .map_err(|err| {
        DecodeError::WebDecoder(super::webcodecs::WebError::Decoding(format!(
            "Failed to create VideoFrame from ImageBitmap: {}",
            string_from_js_value(&err)
        )))
    })?;

    Ok(Frame {
        content: FrameContent::WebVideoFrame(super::webcodecs::WebVideoFrame(video_frame)),
        info: frame_info(chunk),
    })
}

fn decode_image_on_cpu(chunk: Chunk, mime_type: &str) -> Result<Frame> {
    let image_format = image::ImageFormat::from_mime_type(mime_type)
        .or_else(|| image::guess_format(chunk.data.as_slice()).ok())
        .ok_or(DecodeError::WaitingForCodecDetails)?;

    let info = frame_info(&chunk);

    let mut reader = image::ImageReader::new(std::io::Cursor::new(chunk.data));
    reader.set_format(image_format);

    let content = super::image_decoder::decode_to_decoded_frame_content(reader)?;

    Ok(Frame {
        content: FrameContent::Decoded(content),
        info,
    })
}

/// Web APIs can handle 16bit images, but will always give us RGBA8 data in return.
///
/// We need to preserve both single-channelness and 16-bitness, so we have to decode those on the CPU instead of using browser decoding.
/// Single channel is important because otherwise our color-map application doesn't work.
/// We could also work around this by just flagging the data as "actually grayscale", but that's a bit more hacky
/// and we have to deal with >8bit images anyways.
fn should_decode_on_cpu(
    bit_depth: Option<u8>,
    chroma_subsampling: Option<ChromaSubsamplingModes>,
) -> bool {
    // `None` means GOP detection couldn't determine the metadata yet, so take the conservative
    // path and keep the data intact.
    bit_depth.is_none_or(|bit_depth| bit_depth > 8)
        || chroma_subsampling.is_none_or(|chroma_subsampling| {
            chroma_subsampling == ChromaSubsamplingModes::Monochrome
        })
}
