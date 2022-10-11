use egui::{Color32, ColorImage};
use egui_extras::RetainedImage;

use image::DynamicImage;
use re_log_types::*;

#[derive(Default)]
pub struct ImageCache {
    images: nohash_hasher::IntMap<MsgId, CachedImage>,
    memory_used: u64,
    generation: u64,
}

// TODO: the tensor_inserter now precludes passing a reference through,
// causing unncessary clones. This needs to be returned to a reference
// compatible form.
impl ImageCache {
    pub fn get_pair(
        &mut self,
        msg_id: &MsgId,
        tensor_inserter: impl FnOnce() -> Tensor,
    ) -> (&DynamicImage, &RetainedImage) {
        let ci = self.images.entry(*msg_id).or_insert_with(|| {
            // TODO(emilk): proper debug name for images
            let ci = CachedImage::from_tensor(format!("{msg_id:?}"), &tensor_inserter());
            self.memory_used += ci.memory_used;
            ci
        });
        ci.last_use_generation = self.generation;
        (&ci.dynamic_img, &ci.retained_img)
    }

    pub fn get(
        &mut self,
        msg_id: &MsgId,
        tensor_insert: impl FnOnce() -> Tensor,
    ) -> &RetainedImage {
        self.get_pair(msg_id, tensor_insert).1
    }

    /// Call once per frame to (potentially) flush the cache.
    pub fn new_frame(&mut self, max_memory_use: u64) {
        if self.memory_used > max_memory_use {
            let before = self.memory_used;
            self.flush();
            re_log::debug!(
                "Flushed image cache. Before: {:.2} GB. After: {:.2} GB",
                before as f64 / 1e9,
                self.memory_used as f64 / 1e9,
            );
        }

        self.generation += 1;
    }

    fn flush(&mut self) {
        crate::profile_function!();
        // Very agressively flush everything not used in this frame
        self.images.retain(|_, ci| {
            let retain = ci.last_use_generation == self.generation;
            if !retain {
                self.memory_used -= ci.memory_used;
            }
            retain
        });
    }
}

struct CachedImage {
    /// For egui
    retained_img: RetainedImage,

    /// For easily zooming into it in the UI
    dynamic_img: DynamicImage,

    /// Total memory used by this image.
    memory_used: u64,

    /// When [`ImageCache::generation`] was we last used?
    last_use_generation: u64,
}

impl CachedImage {
    fn from_tensor(debug_name: String, tensor: &Tensor) -> Self {
        crate::profile_function!();
        let dynamic_img = match tensor_to_dynamic_image(tensor) {
            Ok(dynamic_image) => dynamic_image,
            Err(err) => {
                re_log::warn!("Bad image {debug_name:?}: {}", re_error::format(&err));
                let error_img = image::RgbImage::from_pixel(1, 1, image::Rgb([255, 0, 255]));
                DynamicImage::ImageRgb8(error_img)
            }
        };
        let egui_color_image = dynamic_image_to_egui_color_image(&dynamic_img);

        let memory_used = egui_color_image.pixels.len() * std::mem::size_of::<egui::Color32>()
            + dynamic_img.as_bytes().len();

        let retained_img = RetainedImage::from_color_image(debug_name, egui_color_image);

        Self {
            dynamic_img,
            retained_img,
            memory_used: memory_used as u64,
            last_use_generation: 0,
        }
    }
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
        u32::try_from(shape[0].size).context("tensor too large")?,
        u32::try_from(shape[1].size).context("tensor too large")?,
    ];
    let depth = if shape.len() == 2 { 1 } else { shape[2].size };

    anyhow::ensure!(
        depth == 1 || depth == 3 || depth == 4,
        "Expected depth of 1,3,4 (gray, RGB, RGBA), found {depth:?}. Tensor shape: {shape:?}"
    );

    type Gray16Image = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;
    type Rgb16Image = image::ImageBuffer<image::Rgb<u16>, Vec<u16>>;
    type Rgba16Image = image::ImageBuffer<image::Rgba<u16>, Vec<u16>>;

    use egui::epaint::color::gamma_u8_from_linear_f32;
    use egui::epaint::color::linear_u8_from_linear_f32;

    match &tensor.data {
        TensorDataStore::Dense(bytes) => {
            anyhow::ensure!(
                bytes.len() as u64 == tensor.len() * tensor.dtype.size(),
                "Tensor data length doesn't match tensor shape and dtype"
            );

            match (depth, tensor.dtype) {
                (1, TensorDataType::U8) => {
                    // TODO(emilk): we should read some meta-data to check if this is luminance or alpha.
                    image::GrayImage::from_raw(width, height, bytes.to_vec())
                        .context("Bad Luminance8")
                        .map(DynamicImage::ImageLuma8)
                }
                (1, TensorDataType::U16) => {
                    // TODO(emilk): we should read some meta-data to check if this is luminance or alpha.
                    Gray16Image::from_raw(width, height, bytemuck::cast_slice(bytes).to_vec())
                        .context("Bad Luminance16")
                        .map(DynamicImage::ImageLuma16)
                }
                (1, TensorDataType::F32) => {
                    let assume_depth = true; // TODO(emilk): we should read some meta-data to check if this is luminance, alpha or a depth map.

                    if assume_depth {
                        if bytes.is_empty() {
                            Ok(DynamicImage::ImageLuma16(Gray16Image::default()))
                        } else {
                            let floats = bytemuck::cast_slice(bytes);

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

                            anyhow::ensure!(
                                min.is_finite() && max.is_finite(),
                                "Depth image had non-finite values"
                            );

                            if min == max {
                                // Uniform image. We can't remap it to a 0-1 range, so do whatever:
                                let ints = floats.iter().map(|&float| float as u16).collect();
                                Gray16Image::from_raw(width, height, ints)
                                    .context("Bad Luminance16")
                                    .map(DynamicImage::ImageLuma16)
                            } else {
                                let ints = floats
                                    .iter()
                                    .map(|&float| {
                                        egui::remap(float, min..=max, 0.0..=65535.0) as u16
                                    })
                                    .collect();

                                Gray16Image::from_raw(width, height, ints)
                                    .context("Bad Luminance16")
                                    .map(DynamicImage::ImageLuma16)
                            }
                        }
                    } else {
                        let l: &[f32] = bytemuck::cast_slice(bytes);
                        let colors: Vec<u8> =
                            l.iter().copied().map(linear_u8_from_linear_f32).collect();
                        image::GrayImage::from_raw(width, height, colors)
                            .context("Bad Luminance f32")
                            .map(DynamicImage::ImageLuma8)
                    }
                }

                (3, TensorDataType::U8) => image::RgbImage::from_raw(width, height, bytes.to_vec())
                    .context("Bad RGB8")
                    .map(DynamicImage::ImageRgb8),
                (3, TensorDataType::U16) => {
                    Rgb16Image::from_raw(width, height, bytemuck::cast_slice(bytes).to_vec())
                        .context("Bad RGB16 image")
                        .map(DynamicImage::ImageRgb16)
                }
                (3, TensorDataType::F32) => {
                    let rgb: &[[f32; 3]] = bytemuck::cast_slice(bytes);
                    let colors: Vec<u8> = rgb
                        .iter()
                        .flat_map(|&[r, g, b]| {
                            let r = gamma_u8_from_linear_f32(r);
                            let g = gamma_u8_from_linear_f32(g);
                            let b = gamma_u8_from_linear_f32(b);
                            [r, g, b]
                        })
                        .collect();
                    image::RgbImage::from_raw(width, height, colors)
                        .context("Bad RGB f32")
                        .map(DynamicImage::ImageRgb8)
                }

                (4, TensorDataType::U8) => {
                    image::RgbaImage::from_raw(width, height, bytes.to_vec())
                        .context("Bad RGBA8")
                        .map(DynamicImage::ImageRgba8)
                }
                (4, TensorDataType::U16) => {
                    Rgba16Image::from_raw(width, height, bytemuck::cast_slice(bytes).to_vec())
                        .context("Bad RGBA16 image")
                        .map(DynamicImage::ImageRgba16)
                }
                (4, TensorDataType::F32) => {
                    let rgba: &[[f32; 4]] = bytemuck::cast_slice(bytes);
                    let colors: Vec<u8> = rgba
                        .iter()
                        .flat_map(|&[r, g, b, a]| {
                            let r = gamma_u8_from_linear_f32(r);
                            let g = gamma_u8_from_linear_f32(g);
                            let b = gamma_u8_from_linear_f32(b);
                            let a = linear_u8_from_linear_f32(a);
                            [r, g, b, a]
                        })
                        .collect();
                    image::RgbaImage::from_raw(width, height, colors)
                        .context("Bad RGBA f32")
                        .map(DynamicImage::ImageRgba8)
                }

                (_depth, dtype) => {
                    anyhow::bail!(
                        "Don't know how to turn a tensor of shape={shape:?} and dtype={dtype:?} into an image"
                    )
                }
            }
        }

        TensorDataStore::Jpeg(bytes) => {
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
