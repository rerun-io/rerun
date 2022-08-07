use egui::{Color32, ColorImage};
use egui_extras::RetainedImage;

use image::DynamicImage;
use re_log_types::*;

#[derive(Default)]
pub struct ImageCache {
    images: nohash_hasher::IntMap<LogId, (DynamicImage, RetainedImage)>,
}

impl ImageCache {
    pub fn get_pair(&mut self, log_id: &LogId, tensor: &Tensor) -> &(DynamicImage, RetainedImage) {
        self.images
            .entry(*log_id)
            .or_insert_with(|| tensor_to_image_pair(format!("{log_id:?}"), tensor))
        // TODO(emilk): better debug name
    }

    pub fn get(&mut self, log_id: &LogId, tensor: &Tensor) -> &RetainedImage {
        &self.get_pair(log_id, tensor).1
    }

    // pub fn get_dynamic_image(&mut self, log_id: &LogId, image: &Image) -> &DynamicImage {
    //     &self.get_pair(log_id, image).0
    // }
}

fn tensor_to_image_pair(debug_name: String, tensor: &Tensor) -> (DynamicImage, RetainedImage) {
    crate::profile_function!();
    let dynamic_image = match tensor_to_dynamic_image(tensor) {
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

fn tensor_to_dynamic_image(tensor: &Tensor) -> anyhow::Result<DynamicImage> {
    crate::profile_function!();
    use anyhow::Context as _;

    let shape = &tensor.shape;

    anyhow::ensure!(
        shape.len() == 2 || shape.len() == 3,
        "Expected a 2D or 3D tensor, got {shape:?}",
    );

    let [height, width] = [
        u32::try_from(shape[0]).context("tensor too large")?,
        u32::try_from(shape[1]).context("tensor tool large")?,
    ];
    let depth = if shape.len() == 2 { 1 } else { shape[2] };

    anyhow::ensure!(
        depth == 1 || depth == 3 || depth == 4,
        "Expected depth of 1,3,4 (gray, RGB, RGBA), found {depth:?}. Tensor shape: {shape:?}"
    );

    type Gray16Image = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;

    match &tensor.data {
        TensorData::Dense(bytes) => {
            if depth == 1 && tensor.dtype == TensorDataType::U8 {
                // TODO(emilk): we should read some meta-data to check if this is luminance or alpha.
                image::GrayImage::from_raw(width, height, bytes.clone())
                    .context("Bad Luminance8")
                    .map(DynamicImage::ImageLuma8)
            } else if depth == 1 && tensor.dtype == TensorDataType::U16 {
                // TODO(emilk): we should read some meta-data to check if this is luminance or alpha.
                Gray16Image::from_raw(width, height, bytemuck::cast_slice(bytes).to_vec())
                    .context("Bad Luminance16")
                    .map(DynamicImage::ImageLuma16)
            } else if depth == 3 && tensor.dtype == TensorDataType::U8 {
                image::RgbImage::from_raw(width, height, bytes.clone())
                    .context("Bad Rgb8")
                    .map(DynamicImage::ImageRgb8)
            } else if depth == 4 && tensor.dtype == TensorDataType::U8 {
                image::RgbaImage::from_raw(width, height, bytes.clone())
                    .context("Bad Rgba8")
                    .map(DynamicImage::ImageRgba8)
            } else if depth == 1 && tensor.dtype == TensorDataType::F32 {
                // Maybe a depth map?
                if let TensorData::Dense(bytes) = &tensor.data {
                    if let Ok(floats) = bytemuck::try_cast_slice(bytes) {
                        // Convert to u16 so we can put them in an image.
                        // TODO(emilk): Eventually we want a renderer that can show f32 images natively.
                        // One big downside of the approach below is that if we have two dept images
                        // in the same range, they cannot be visually compared with each other,
                        // because their individual max-depths will be scaled to 65535.

                        let mut min = f32::INFINITY;
                        let mut max = f32::NEG_INFINITY;
                        for &float in floats {
                            min = min.min(float);
                            max = max.max(float);
                        }

                        if min < max && min.is_finite() && max.is_finite() {
                            let ints = floats
                                .iter()
                                .map(|&float| {
                                    let int = egui::remap(float, min..=max, 0.0..=65535.0);
                                    int as u16
                                })
                                .collect();

                            return Gray16Image::from_raw(width, height, ints)
                                .context("Bad Luminance16")
                                .map(DynamicImage::ImageLuma16);
                        }
                    }
                }

                anyhow::bail!(
                    "Don't know how to turn a tensor of shape={:?} and dtype={:?} into an image",
                    shape,
                    tensor.dtype
                )
            } else {
                anyhow::bail!(
                    "Don't know how to turn a tensor of shape={:?} and dtype={:?} into an image",
                    shape,
                    tensor.dtype
                )
            }
        }

        TensorData::Jpeg(bytes) => {
            crate::profile_scope!("Decode JPEG");
            use image::io::Reader as ImageReader;
            let mut reader = ImageReader::new(std::io::Cursor::new(bytes));
            reader.set_format(image::ImageFormat::Jpeg);
            // TODO(emilk): handle grayscale JPEG:s (depth == 1)
            let img = reader.decode()?.into_rgb8();

            if depth != 3 || img.width() != width || img.height() != height {
                anyhow::bail!(
                    "Tensor shape ({shape:?}) did not match jpeg dimensions ({}x{})",
                    img.width(),
                    img.height()
                )
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
        DynamicImage::ImageLuma16(gray) => ColorImage {
            size: [gray.width() as _, gray.height() as _],
            pixels: gray
                .pixels()
                .map(|pixel| Color32::from_gray((pixel[0] / 256) as u8))
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
