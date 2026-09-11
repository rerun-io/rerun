use crate::decode::sync_decoder_wrapper::SyncDecoder;

/// Decodes each chunk's data as one complete TIFF file.
///
/// See [`decode_tiff`] for the supported sample formats.
pub struct TiffDecoder;

impl SyncDecoder for TiffDecoder {
    fn submit_chunk(
        &mut self,
        should_stop: &std::sync::atomic::AtomicBool,
        chunk: super::Chunk,
        output_sender: &re_quota_channel::Sender<super::FrameResult>,
    ) {
        if should_stop.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        let decoded = match decode_tiff(&chunk.data) {
            Ok(decoded) => decoded,
            Err(err) => {
                let _send_error = output_sender.send(Err(err));
                return;
            }
        };

        let content = cfg_select! {
            target_arch = "wasm32" => { super::FrameContent::Decoded(decoded) }
            _ => { decoded }
        };

        let _send_error = output_sender.send(Ok(super::Frame {
            content,
            info: super::FrameInfo {
                is_sync: Some(true),
                frame_nr: Some(chunk.frame_nr),
                source: Some(chunk.source),
                presentation_timestamp: chunk.presentation_timestamp,
                duration: chunk.duration,
                latest_decode_timestamp: Some(chunk.decode_timestamp),
            },
        }));
    }

    fn reset(&mut self, _video_data_description: &crate::VideoDataDescription) {}
}

/// Decode a single-channel TIFF into a raw frame.
///
/// Only grayscale TIFFs with U8, U16, or F32 samples decode, mapping to
/// [`super::PixelFormat::L8`], [`super::PixelFormat::L16`], and
/// [`super::PixelFormat::R32Float`] respectively.
pub fn decode_tiff(data: &[u8]) -> Result<super::DecodedFrameContent, super::DecodeError> {
    use tiff::decoder::DecodingResult;

    let image_decode_err =
        |err: &dyn std::fmt::Display| super::DecodeError::ImageDecoder(format!("TIFF: {err}"));

    let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(data))
        .map_err(|err| image_decode_err(&err))?;

    let color_type = decoder.colortype().map_err(|err| image_decode_err(&err))?;
    if !matches!(color_type, tiff::ColorType::Gray(_)) {
        return Err(super::DecodeError::ImageDecoder(format!(
            "TIFF: only single-channel (grayscale) images are supported, got {color_type:?}"
        )));
    }

    let (width, height) = decoder.dimensions().map_err(|err| image_decode_err(&err))?;

    let image = decoder.read_image().map_err(|err| image_decode_err(&err))?;
    let (data, format): (Vec<u8>, super::PixelFormat) = match image {
        DecodingResult::U8(data) => (data, super::PixelFormat::L8),
        DecodingResult::U16(data) => (
            bytemuck::cast_slice(&data).to_vec(),
            super::PixelFormat::L16,
        ),
        DecodingResult::F32(data) => (
            bytemuck::cast_slice(&data).to_vec(),
            super::PixelFormat::R32Float,
        ),
        DecodingResult::U32(_)
        | DecodingResult::U64(_)
        | DecodingResult::F16(_)
        | DecodingResult::F64(_)
        | DecodingResult::I8(_)
        | DecodingResult::I16(_)
        | DecodingResult::I32(_)
        | DecodingResult::I64(_) => {
            return Err(super::DecodeError::ImageDecoder(
                "TIFF: only U8, U16, and F32 samples are supported".to_owned(),
            ));
        }
    };

    Ok(super::DecodedFrameContent {
        data,
        width,
        height,
        format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_gray_f32_tiff(values: &[f32], width: u32, height: u32) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut encoder = tiff::encoder::TiffEncoder::new(&mut buf).unwrap();
        encoder
            .write_image::<tiff::encoder::colortype::Gray32Float>(width, height, values)
            .unwrap();
        buf.into_inner()
    }

    #[test]
    fn decodes_gray_f32_tiff() {
        let values = [0.0_f32, 0.5, 1.5, 2.0];
        let encoded = encode_gray_f32_tiff(&values, 2, 2);

        let decoded = decode_tiff(&encoded).unwrap();
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 2);
        assert!(matches!(decoded.format, crate::PixelFormat::R32Float));
        let pixels: &[f32] = bytemuck::cast_slice(&decoded.data);
        assert_eq!(pixels, &values);
    }

    #[test]
    fn rejects_rgb_tiff() {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut encoder = tiff::encoder::TiffEncoder::new(&mut buf).unwrap();
        encoder
            .write_image::<tiff::encoder::colortype::RGB8>(1, 1, &[0_u8, 0, 0])
            .unwrap();

        let Err(err) = decode_tiff(&buf.into_inner()) else {
            panic!("RGB TIFF must be rejected");
        };
        assert!(err.to_string().contains("single-channel"));
    }
}
