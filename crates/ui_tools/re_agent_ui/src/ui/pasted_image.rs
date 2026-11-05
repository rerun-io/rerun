//! Encoding of images pasted or dropped into the composer, and decoding of the ones already
//! attached to a queued prompt.

use re_agent::PromptImage;

/// Longest edge, in pixels, an attached image is scaled down to before it is sent.
///
/// 1568 is where Anthropic's API stops scaling an image down itself, so anything larger is paid
/// for in upload time and tokens without reaching the model. It still reads UI text.
const MAX_EDGE: u32 = 1568;

/// The MIME type attached images are encoded as. PNG keeps screenshot text sharp.
const MIME_TYPE: &str = "image/png";

/// Encodes a pasted image for sending, and returns it alongside the pixels to preview it with.
///
/// The preview is the encoded image decoded again, not the original, so what the composer shows
/// is what the agent receives. It is full size: the composer is what scales it down.
pub fn encode(source: &egui::ColorImage) -> Option<(PromptImage, egui::ColorImage)> {
    re_tracing::profile_function!();

    let [width, height] = source.size;
    let (width, height) = (u32::try_from(width).ok()?, u32::try_from(height).ok()?);

    // `ColorImage` holds premultiplied alpha, which PNG does not: unmultiply before encoding, or
    // every semi-transparent pixel darkens.
    let unmultiplied: Vec<u8> = source
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_srgba_unmultiplied())
        .collect();

    let rgba = image::RgbaImage::from_raw(width, height, unmultiplied)?;
    encode_image(image::DynamicImage::ImageRgba8(rgba)).ok()
}

/// Encodes the contents of an image file (PNG or JPEG) for sending.
pub fn encode_image_file(bytes: &[u8]) -> Result<PromptImage, image::ImageError> {
    re_tracing::profile_function!();

    let (image, _preview) = encode_image(image::load_from_memory(bytes)?)?;
    Ok(image)
}

fn encode_image(
    mut decoded: image::DynamicImage,
) -> image::ImageResult<(PromptImage, egui::ColorImage)> {
    if MAX_EDGE < decoded.width().max(decoded.height()) {
        re_tracing::profile_scope!("resize");
        decoded = decoded.resize(MAX_EDGE, MAX_EDGE, image::imageops::FilterType::CatmullRom);
    }

    let mut bytes = Vec::new();
    decoded.write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )?;

    let resolution = [decoded.width() as usize, decoded.height() as usize];
    let preview = egui::ColorImage::from_rgba_unmultiplied(resolution, decoded.to_rgba8().as_raw());

    Ok((
        PromptImage {
            bytes: bytes.into(),
            mime_type: MIME_TYPE.to_owned(),
            resolution,
        },
        preview,
    ))
}

/// Decodes an already-encoded image, to show a prompt taken back out of the queue.
pub fn decode(image: &PromptImage) -> Option<egui::ColorImage> {
    re_tracing::profile_function!();

    let decoded = image::load_from_memory(&image.bytes).ok()?;
    let size = [decoded.width() as usize, decoded.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        decoded.to_rgba8().as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_image_file_scales_down_a_large_jpeg() {
        let jpeg = image::DynamicImage::new_rgb8(2 * MAX_EDGE, MAX_EDGE);
        let mut bytes = Vec::new();
        jpeg.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Jpeg,
        )
        .expect("encoding a blank JPEG never fails");

        let image = encode_image_file(&bytes).expect("a valid JPEG");
        assert_eq!(image.mime_type, MIME_TYPE);
        assert_eq!(image.resolution, [MAX_EDGE as usize, MAX_EDGE as usize / 2]);
    }

    #[test]
    fn encode_image_file_rejects_garbage() {
        assert!(encode_image_file(b"not an image").is_err());
    }
}
