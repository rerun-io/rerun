use crate::{PixelFormat, decode::async_decoder_wrapper::SyncDecoder};

pub struct SyncImageDecoder {
    image_format: image::ImageFormat,
}

impl SyncImageDecoder {
    pub fn try_new(descr: &crate::VideoDataDescription) -> Option<Self> {
        Some(Self {
            image_format: image::ImageFormat::from_mime_type(descr.image_codec_mime_type()?)?,
        })
    }

    pub fn mime_type(&self) -> &'static str {
        self.image_format.to_mime_type()
    }
}

impl SyncDecoder for SyncImageDecoder {
    // TODO(isse): We could potentially cache decoded blobs, but that's missing some things:
    // - A way to purge the cache, i.e have a purge function on video decoders that gets called from `VideoStreamCache`?
    // - Some unique hash to identify blobs. We don't want to hash the whole blob
    //   to get that. For `StoredBlobCacheKey` we use row id + component identifier,
    //   but we don't have an obvious way to pass that here. Could potentially
    //   use the samples `source_id` + `byte_span`.
    fn submit_chunk(
        &mut self,
        should_stop: &std::sync::atomic::AtomicBool,
        chunk: super::Chunk,
        output_sender: &re_quota_channel::Sender<super::FrameResult>,
    ) {
        if should_stop.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        let mut reader = image::ImageReader::new(std::io::Cursor::new(chunk.data));

        reader.set_format(self.image_format);

        let content = match decode_to_frame_content(reader) {
            Ok(content) => content,
            Err(err) => {
                let _send_error = output_sender.send(crate::FrameResult::Err(err));
                return;
            }
        };

        let _send_error = output_sender.send(crate::FrameResult::Ok(crate::Frame {
            content,
            info: crate::FrameInfo {
                is_sync: Some(true),
                sample_idx: Some(chunk.sample_idx),
                frame_nr: Some(chunk.frame_nr),
                presentation_timestamp: chunk.presentation_timestamp,
                duration: chunk.duration,
                latest_decode_timestamp: Some(chunk.decode_timestamp),
            },
        }));
    }

    fn reset(&mut self, descr: &crate::VideoDataDescription) {
        if let Some(new) = Self::try_new(descr) {
            *self = new;
        }
    }
}

fn decode_to_frame_content(
    reader: image::ImageReader<std::io::Cursor<Vec<u8>>>,
) -> Result<crate::FrameContent, crate::DecodeError> {
    let dynamic_image = reader
        .decode()
        .map_err(|err| crate::DecodeError::ImageDecoder(err.to_string()))?;

    let converted_rgb;
    let converted_rgba;
    let (data, (width, height), format): (&[u8], (u32, u32), PixelFormat) = match &dynamic_image {
        image::DynamicImage::ImageLuma8(image) => {
            (image.as_raw(), image.dimensions(), PixelFormat::L8)
        }
        image::DynamicImage::ImageLumaA8(image) => {
            (image.as_raw(), image.dimensions(), PixelFormat::L8)
        }
        image::DynamicImage::ImageRgb8(image) => {
            (image.as_raw(), image.dimensions(), PixelFormat::Rgb8Unorm)
        }
        image::DynamicImage::ImageRgba8(image) => {
            (image.as_raw(), image.dimensions(), PixelFormat::Rgba8Unorm)
        }
        image::DynamicImage::ImageLuma16(image) => (
            bytemuck::cast_slice(image.as_raw()),
            image.dimensions(),
            PixelFormat::L16,
        ),
        image::DynamicImage::ImageLumaA16(image) => (
            bytemuck::cast_slice(image.as_raw()),
            image.dimensions(),
            PixelFormat::L16,
        ),
        image::DynamicImage::ImageRgb16(_) | image::DynamicImage::ImageRgb32F(_) => {
            converted_rgb = dynamic_image.to_rgb8();

            (
                converted_rgb.as_raw(),
                converted_rgb.dimensions(),
                PixelFormat::Rgb8Unorm,
            )
        }
        image::DynamicImage::ImageRgba16(_) | image::DynamicImage::ImageRgba32F(_) => {
            converted_rgba = dynamic_image.to_rgba8();

            (
                converted_rgba.as_raw(),
                converted_rgba.dimensions(),
                PixelFormat::Rgba8Unorm,
            )
        }
        _ => {
            return Err(crate::DecodeError::ImageDecoder(
                "Unsupported image layout".to_owned(),
            ));
        }
    };

    Ok(crate::FrameContent {
        data: data.to_owned(),
        width,
        height,
        format,
    })
}
