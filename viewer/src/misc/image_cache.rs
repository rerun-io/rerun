use egui::{Color32, ColorImage};
use egui_extras::RetainedImage;

use image::DynamicImage;
use log_types::*;

#[derive(Default)]
pub struct ImageCache {
    images: nohash_hasher::IntMap<LogId, (DynamicImage, RetainedImage)>,
}

impl ImageCache {
    pub fn get_pair(&mut self, log_id: &LogId, rr_image: &Image) -> &(DynamicImage, RetainedImage) {
        self.images
            .entry(*log_id)
            .or_insert_with(|| rr_image_to_image_pair(format!("{log_id:?}"), rr_image))
        // TODO: better debug name
    }

    pub fn get(&mut self, log_id: &LogId, image: &Image) -> &RetainedImage {
        &self.get_pair(log_id, image).1
    }

    // pub fn get_dynamic_image(&mut self, log_id: &LogId, image: &Image) -> &DynamicImage {
    //     &self.get_pair(log_id, image).0
    // }
}

fn rr_image_to_image_pair(debug_name: String, rr_image: &Image) -> (DynamicImage, RetainedImage) {
    crate::profile_function!();
    let dynamic_image = match rr_image_to_dynamic_image(rr_image) {
        Ok(dynamic_image) => dynamic_image,
        Err(err) => {
            tracing::warn!("Bad image {debug_name:?}: {err}");
            DynamicImage::ImageRgb8(image::RgbImage::from_pixel(1, 1, image::Rgb([255, 0, 255])))
        }
    };
    let egui_color_image = dynamic_image_to_egui_color_image(&dynamic_image);
    let retrained_iamge = RetainedImage::from_color_image(debug_name, egui_color_image);
    (dynamic_image, retrained_iamge)
}

fn rr_image_to_dynamic_image(rr_image: &Image) -> anyhow::Result<DynamicImage> {
    crate::profile_function!();
    use anyhow::Context as _;

    let [w, h] = rr_image.size;

    match rr_image.format {
        log_types::ImageFormat::Luminance8 => {
            image::GrayImage::from_raw(w, h, rr_image.data.clone())
                .context("Bad Luminance8")
                .map(DynamicImage::ImageLuma8)
        }

        log_types::ImageFormat::Rgb8 => image::RgbImage::from_raw(w, h, rr_image.data.clone())
            .context("Bad Rgb8")
            .map(DynamicImage::ImageRgb8),

        log_types::ImageFormat::Rgba8 => image::RgbaImage::from_raw(w, h, rr_image.data.clone())
            .context("Bad Rgba8")
            .map(DynamicImage::ImageRgba8),

        log_types::ImageFormat::Jpeg => {
            crate::profile_scope!("Decode JPEG");
            use image::io::Reader as ImageReader;
            let mut reader = ImageReader::new(std::io::Cursor::new(&rr_image.data));
            reader.set_format(image::ImageFormat::Jpeg);
            let img = reader.decode()?.to_rgb8();

            let size = [img.width(), img.height()];
            if size != rr_image.size {
                tracing::warn!(
                    "Declared image size ({}x{}) does not match jpeg size ({}x{})",
                    rr_image.size[0],
                    rr_image.size[1],
                    size[0],
                    size[1],
                );
            }

            Ok(DynamicImage::ImageRgb8(img))
        }
    }
}

fn dynamic_image_to_egui_color_image(dynamic_image: &DynamicImage) -> ColorImage {
    crate::profile_function!();
    match dynamic_image {
        DynamicImage::ImageLuma8(gray) => ColorImage {
            size: [gray.width() as _, gray.height() as _],
            pixels: gray
                .pixels()
                .map(|pixel| Color32::from_gray(pixel[0]))
                .collect(),
        },
        DynamicImage::ImageRgb8(rgb) => ColorImage {
            size: [rgb.width() as _, rgb.height() as _],
            pixels: rgb
                .pixels()
                .map(|rgb| Color32::from_rgb(rgb[0], rgb[1], rgb[2]))
                .collect(),
        },
        DynamicImage::ImageRgba8(rgba) => ColorImage {
            size: [rgba.width() as _, rgba.height() as _],
            pixels: rgba
                .pixels()
                .map(|rgba| Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
                .collect(),
        },
        _ => dynamic_image_to_egui_color_image(&DynamicImage::ImageRgba8(dynamic_image.to_rgba8())),
    }
}

// fn rr_image_to_retained_image(rr_image: &Image) -> RetainedImage {
//     crate::profile_function!();
//     match rr_image_to_rgba_unultiplied(rr_image) {
//         Ok((size, rgba)) => {
//             let pixels = rgba
//                 .chunks(4)
//                 .map(|chunk| {
//                     let [r, g, b, a] = [chunk[0], chunk[1], chunk[2], chunk[3]];
//                     if a == 255 {
//                         Color32::from_rgb(r, g, b) // common-case optimization; inlined
//                     } else {
//                         Color32::from_rgba_unmultiplied(r, g, b, a)
//                     }
//                 })
//                 .collect();
//             let size = [size[0] as _, size[1] as _];
//             let color_image = egui::ColorImage { size, pixels };
//             RetainedImage::from_color_image("image", color_image)
//         }
//         Err(err) => {
//             tracing::warn!("Bad image: {err}"); // TODO: path, log id, SOMETHING!
//             let color_image = egui::ColorImage {
//                 size: [1, 1],
//                 pixels: vec![Color32::from_rgb(255, 0, 255)],
//             };
//             RetainedImage::from_color_image("image", color_image)
//         }
//     }
// }

// pub fn rr_image_to_rgba_unultiplied(
//     rr_image: &log_types::Image,
// ) -> anyhow::Result<([u32; 2], Vec<u8>)> {
//     crate::profile_function!();
//     match rr_image.format {
//         log_types::ImageFormat::Luminance8 => {
//             let rgba: Vec<u8> = rr_image
//                 .data
//                 .iter()
//                 .flat_map(|&l| [l, l, l, 255])
//                 .collect();
//             sanity_check_size(rr_image.size, &rgba)?;
//             Ok((rr_image.size, rgba))
//         }
//         log_types::ImageFormat::Rgba8 => {
//             let rgba = rr_image.data.clone();
//             sanity_check_size(rr_image.size, &rgba)?;
//             Ok((rr_image.size, rgba))
//         }
//         log_types::ImageFormat::Jpeg => {
//             crate::profile_scope!("Decode JPEG");
//             use image::io::Reader as ImageReader;
//             let mut reader = ImageReader::new(std::io::Cursor::new(&rr_image.data));
//             reader.set_format(image::ImageFormat::Jpeg);
//             let img = reader.decode()?.to_rgba8();
//             let rgba = img.to_vec();
//             let size = [img.width(), img.height()];

//             if size != rr_image.size {
//                 tracing::warn!(
//                     "Declared image size ({}x{}) does not match jpeg size ({}x{})",
//                     rr_image.size[0],
//                     rr_image.size[1],
//                     size[0],
//                     size[1],
//                 );
//             }

//             sanity_check_size(size, &rgba)?;
//             Ok((size, rgba))
//         }
//     }
// }

// fn sanity_check_size([w, h]: [u32; 2], rgba: &[u8]) -> anyhow::Result<()> {
//     anyhow::ensure!(
//         (w as usize) * (h as usize) * 4 == rgba.len(),
//         "Image had a size of {w}x{h} (={}), but had {} pixels",
//         w * h,
//         rgba.len() / 4
//     );
//     Ok(())
// }
