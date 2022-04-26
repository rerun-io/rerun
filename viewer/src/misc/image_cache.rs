use egui::Color32;
use egui_extras::RetainedImage;

use log_types::*;

#[derive(Default)]
pub struct ImageCache {
    images: nohash_hasher::IntMap<LogId, RetainedImage>,
}

impl ImageCache {
    pub fn get(&mut self, log_id: &LogId, image: &Image) -> &RetainedImage {
        self.images
            .entry(*log_id)
            .or_insert_with(|| to_egui_image(image))
    }
}

fn to_egui_image(image: &Image) -> RetainedImage {
    crate::profile_function!();
    match to_rgba_unultiplied(image) {
        Ok((size, rgba)) => {
            let pixels = rgba
                .chunks(4)
                .map(|chunk| {
                    let [r, g, b, a] = [chunk[0], chunk[1], chunk[2], chunk[3]];
                    if a == 255 {
                        Color32::from_rgb(r, g, b) // common-case optimization; inlined
                    } else {
                        Color32::from_rgba_unmultiplied(r, g, b, a)
                    }
                })
                .collect();
            let size = [size[0] as _, size[1] as _];
            let color_image = egui::ColorImage { size, pixels };
            RetainedImage::from_color_image("image", color_image)
        }
        Err(err) => {
            tracing::warn!("Bad image: {err}"); // TODO: path, log id, SOMETHING!

            let color_image = egui::ColorImage {
                size: [1, 1],
                pixels: vec![Color32::from_rgb(255, 0, 255)],
            };
            RetainedImage::from_color_image("image", color_image)
        }
    }
}

pub fn to_rgba_unultiplied(image: &log_types::Image) -> anyhow::Result<([u32; 2], Vec<u8>)> {
    crate::profile_function!();
    match image.format {
        log_types::ImageFormat::Luminance8 => {
            let rgba: Vec<u8> = image.data.iter().flat_map(|&l| [l, l, l, 255]).collect();
            sanity_check_size(image.size, &rgba)?;
            Ok((image.size, rgba))
        }
        log_types::ImageFormat::Rgba8 => {
            let rgba = image.data.clone();
            sanity_check_size(image.size, &rgba)?;
            Ok((image.size, rgba))
        }
        log_types::ImageFormat::Jpeg => {
            crate::profile_scope!("Decode JPEG");
            use image::io::Reader as ImageReader;
            let mut reader = ImageReader::new(std::io::Cursor::new(&image.data));
            reader.set_format(image::ImageFormat::Jpeg);
            let img = reader.decode()?.to_rgba8();
            let rgba = img.to_vec();
            let size = [img.width(), img.height()];

            if size != image.size {
                tracing::warn!(
                    "Declared image size ({}x{}) does not match jpeg size ({}x{})",
                    image.size[0],
                    image.size[1],
                    size[0],
                    size[1],
                );
            }

            sanity_check_size(size, &rgba)?;
            Ok((size, rgba))
        }
    }
}

fn sanity_check_size([w, h]: [u32; 2], rgba: &[u8]) -> anyhow::Result<()> {
    anyhow::ensure!(
        (w as usize) * (h as usize) * 4 == rgba.len(),
        "Image had a size of {w}x{h} (={}), but had {} pixels",
        w * h,
        rgba.len() / 4
    );
    Ok(())
}
