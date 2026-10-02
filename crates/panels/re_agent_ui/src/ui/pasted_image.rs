//! Encoding of images pasted into the composer, and decoding of the ones already attached to a
//! queued prompt.

use re_agent::PromptImage;

/// Longest edge, in pixels, a pasted image is scaled down to before it is sent.
///
/// 1568 is where Anthropic's API stops scaling an image down itself, so anything larger is paid
/// for in upload time and tokens without reaching the model. It still reads UI text.
const MAX_EDGE: u32 = 1568;

/// The MIME type pasted images are encoded as. PNG keeps screenshot text sharp.
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
    let mut decoded = image::DynamicImage::ImageRgba8(rgba);
    if MAX_EDGE < width.max(height) {
        re_tracing::profile_scope!("resize");
        decoded = decoded.resize(MAX_EDGE, MAX_EDGE, image::imageops::FilterType::CatmullRom);
    }

    let mut bytes = Vec::new();
    decoded
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .ok()?;

    let size = [decoded.width() as usize, decoded.height() as usize];
    let preview = egui::ColorImage::from_rgba_unmultiplied(size, decoded.to_rgba8().as_raw());

    Some((
        PromptImage {
            bytes: bytes.into(),
            mime_type: MIME_TYPE.to_owned(),
            size,
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
