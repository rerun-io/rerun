use egui::{Color32, ColorImage};
use egui_extras::RetainedImage;
use image::DynamicImage;
use re_log_types::{MsgId, Tensor, TensorDataMeaning, TensorDataStore, TensorDataType};

use crate::ui::view_2d::{ColorMapping, Legend};

// ---

/// The `TensorImageView` is a wrapper on top of `re_log_types::Tensor`
///
/// It consolidates the common operations of going from the raw tensor storage
/// into an object that can be more natively displayed as an Image.
///
/// The `dynamic_img` and `retained_img` are cached to keep the overhead low.
///
/// In the case of images that leverage a `ColorMapping` this includes conversion from
/// the native Tensor type A -> Color32 which is stored for the cached dynamic /
/// retained images.
pub struct TensorImageView<'store, 'cache> {
    /// Borrowed tensor from the object store
    pub tensor: &'store Tensor,

    /// Legend used to create the view
    pub legend: &'store Option<Legend>,

    /// DynamicImage helper for things like zoom
    pub dynamic_img: &'cache DynamicImage,

    /// For egui
    pub retained_img: &'cache RetainedImage,
}

// Use a MsgIdPair for the cache index so that we don't cache across
// changes to the legend
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ImageCacheKey {
    image_msg_id: MsgId,
    legend_msg_id: Option<MsgId>,
}
impl nohash_hasher::IsEnabled for ImageCacheKey {}

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for ImageCacheKey {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let msg_hash = self.image_msg_id.0.as_u128() as u64;

        let legend_hash = if let Some(legend_msg_id) = self.legend_msg_id {
            (legend_msg_id.0.as_u128() >> 1) as u64
        } else {
            0
        };

        state.write_u64(msg_hash ^ legend_hash);
    }
}

#[derive(Default)]
pub struct ImageCache {
    images: nohash_hasher::IntMap<ImageCacheKey, CachedImage>,
    memory_used: u64,
    generation: u64,
}

impl ImageCache {
    pub(crate) fn get_view_with_legend<'store, 'cache>(
        &'cache mut self,
        msg_id: &MsgId,
        tensor: &'store Tensor,
        legend: &'store Option<Legend>,
    ) -> TensorImageView<'store, 'cache> {
        let ci = self
            .images
            .entry(ImageCacheKey {
                image_msg_id: *msg_id,
                legend_msg_id: legend.as_ref().map(|seg_map| seg_map.msg_id),
            })
            .or_insert_with(|| {
                // TODO(emilk): proper debug name for images
                let ci = CachedImage::from_tensor(format!("{msg_id:?}"), tensor, legend);
                self.memory_used += ci.memory_used;
                ci
            });
        ci.last_use_generation = self.generation;

        TensorImageView::<'store, '_> {
            tensor,
            legend,
            dynamic_img: &ci.dynamic_img,
            retained_img: &ci.retained_img,
        }
    }

    pub(crate) fn get_view<'store, 'cache>(
        &'cache mut self,
        msg_id: &MsgId,
        tensor: &'store Tensor,
    ) -> TensorImageView<'store, 'cache> {
        self.get_view_with_legend(msg_id, tensor, &None)
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
    fn from_tensor(debug_name: String, tensor: &Tensor, legend: &Option<Legend>) -> Self {
        crate::profile_function!();
        let dynamic_img = match tensor_to_dynamic_image(tensor, legend) {
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

        let options = egui::TextureOptions {
            // This is best for low-res depth-images and the like
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Linear,
        };
        let retained_img =
            RetainedImage::from_color_image(debug_name, egui_color_image).with_options(options);

        Self {
            dynamic_img,
            retained_img,
            memory_used: memory_used as u64,
            last_use_generation: 0,
        }
    }
}

fn tensor_to_dynamic_image(
    tensor: &Tensor,
    legend: &Option<Legend>,
) -> anyhow::Result<DynamicImage> {
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

            match (legend, depth, tensor.dtype, tensor.meaning) {
                (Some(legend), 1, TensorDataType::U8, TensorDataMeaning::ClassId) => {
                    // Apply legend mapping to raw bytes interpreted as u8
                    image::RgbaImage::from_raw(
                        width,
                        height,
                        bytes
                            .to_vec()
                            .iter()
                            .flat_map(|p| legend.map_color(*p as u16))
                            .collect(),
                    )
                    .context("Bad RGBA8")
                    .map(DynamicImage::ImageRgba8)
                }
                (Some(legend), 1, TensorDataType::U16, TensorDataMeaning::ClassId) => {
                    // Apply legend mapping to bytes interpreted as u16
                    image::RgbaImage::from_raw(
                        width,
                        height,
                        bytemuck::cast_slice(bytes)
                            .to_vec()
                            .iter()
                            .flat_map(|p| legend.map_color(*p))
                            .collect(),
                    )
                    .context("Bad RGBA8")
                    .map(DynamicImage::ImageRgba8)
                }
                (_, 1, TensorDataType::U8, _) => {
                    // TODO(emilk): we should read some meta-data to check if this is luminance or alpha.
                    image::GrayImage::from_raw(width, height, bytes.to_vec())
                        .context("Bad Luminance8")
                        .map(DynamicImage::ImageLuma8)
                }
                (_, 1, TensorDataType::U16, _) => {
                    // TODO(emilk): we should read some meta-data to check if this is luminance or alpha.
                    Gray16Image::from_raw(width, height, bytemuck::cast_slice(bytes).to_vec())
                        .context("Bad Luminance16")
                        .map(DynamicImage::ImageLuma16)
                }
                (_, 1, TensorDataType::F32, _) => {
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

                (_, 3, TensorDataType::U8, _) => {
                    image::RgbImage::from_raw(width, height, bytes.to_vec())
                        .context("Bad RGB8")
                        .map(DynamicImage::ImageRgb8)
                }
                (_, 3, TensorDataType::U16, _) => {
                    Rgb16Image::from_raw(width, height, bytemuck::cast_slice(bytes).to_vec())
                        .context("Bad RGB16 image")
                        .map(DynamicImage::ImageRgb16)
                }
                (_, 3, TensorDataType::F32, _) => {
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

                (_, 4, TensorDataType::U8, _) => {
                    image::RgbaImage::from_raw(width, height, bytes.to_vec())
                        .context("Bad RGBA8")
                        .map(DynamicImage::ImageRgba8)
                }
                (_, 4, TensorDataType::U16, _) => {
                    Rgba16Image::from_raw(width, height, bytemuck::cast_slice(bytes).to_vec())
                        .context("Bad RGBA16 image")
                        .map(DynamicImage::ImageRgba16)
                }
                (_, 4, TensorDataType::F32, _) => {
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
                (Some(_), _depth, dtype, meaning @ TensorDataMeaning::ClassId) => {
                    anyhow::bail!(
                        "Shape={shape:?} and dtype={dtype:?} is incompatible with meaning={meaning:?}"
                    )
                }

                (_, _depth, dtype, _) => {
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
