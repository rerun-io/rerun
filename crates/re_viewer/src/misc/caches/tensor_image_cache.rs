use std::sync::Arc;

use egui::{Color32, ColorImage};
use egui_extras::RetainedImage;
use image::DynamicImage;
use re_log_types::{
    component_types::{self, ClassId, TensorData, TensorDataMeaning, TensorTrait},
    ClassicTensor, MsgId,
};
use re_renderer::{
    resource_managers::{GpuTexture2DHandle, Texture2DCreationDesc},
    RenderContext,
};

use crate::ui::{Annotations, DefaultColor, MISSING_ANNOTATIONS};

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
    /// Borrowed tensor from the data store
    pub tensor: &'store dyn AsDynamicImage,

    /// Annotations used to create the view
    pub annotations: &'store Arc<Annotations>,

    /// DynamicImage helper for things like zoom
    pub dynamic_img: Option<&'cache DynamicImage>,

    /// For egui
    pub retained_img: Option<&'cache RetainedImage>,

    /// For rendering with re_renderer
    pub texture_handle: Option<GpuTexture2DHandle>,
}

// Use this for the cache index so that we don't cache across
// changes to the annotations
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ImageCacheKey {
    tensor_id: component_types::TensorId,
    annotation_msg_id: MsgId,
}

impl nohash_hasher::IsEnabled for ImageCacheKey {}

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for ImageCacheKey {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let msg_hash = self.tensor_id.0.as_u128() as u64;
        let annotation_hash = (self.annotation_msg_id.as_u128() >> 1) as u64;
        state.write_u64(msg_hash ^ annotation_hash);
    }
}

#[derive(Default)]
pub struct ImageCache {
    images: nohash_hasher::IntMap<ImageCacheKey, CachedImage>,
    memory_used: u64,
    generation: u64,
}

impl ImageCache {
    pub(crate) fn get_view_with_annotations<'store, 'cache>(
        &'cache mut self,
        tensor: &'store dyn AsDynamicImage,
        annotations: &'store Arc<Annotations>,
        render_ctx: &mut RenderContext,
    ) -> TensorImageView<'store, 'cache> {
        let ci = self
            .images
            .entry(ImageCacheKey {
                tensor_id: tensor.id(),
                annotation_msg_id: annotations.msg_id,
            })
            .or_insert_with(|| {
                let debug_name = format!("tensor {:?}", tensor.shape());
                let ci = CachedImage::from_tensor(render_ctx, debug_name, tensor, annotations);
                self.memory_used += ci.memory_used;
                ci
            });
        ci.last_use_generation = self.generation;

        TensorImageView::<'store, '_> {
            tensor,
            annotations,
            dynamic_img: ci.dynamic_img.as_ref(),
            retained_img: ci.retained_img.as_ref(),
            texture_handle: ci.texture_handle.clone(),
        }
    }

    pub(crate) fn get_view<'store, 'cache, T: AsDynamicImage>(
        &'cache mut self,
        tensor: &'store T,
        render_ctx: &mut RenderContext,
    ) -> TensorImageView<'store, 'cache> {
        self.get_view_with_annotations(tensor, &MISSING_ANNOTATIONS, render_ctx)
    }

    /// Call once per frame to (potentially) flush the cache.
    pub fn new_frame(&mut self, max_memory_use: u64) {
        if self.memory_used > max_memory_use {
            self.purge_memory();
        }

        self.generation += 1;
    }

    /// Attempt to free up memory.
    pub fn purge_memory(&mut self) {
        crate::profile_function!();

        // Very aggressively flush everything not used in this frame

        let before = self.memory_used;

        self.images.retain(|_, ci| {
            let retain = ci.last_use_generation == self.generation;
            if !retain {
                self.memory_used -= ci.memory_used;
            }
            retain
        });

        re_log::debug!(
            "Flushed image cache. Before: {:.2} GB. After: {:.2} GB",
            before as f64 / 1e9,
            self.memory_used as f64 / 1e9,
        );
    }
}

struct CachedImage {
    /// For egui. `None` if the tensor was not a valid image.
    /// TODO(andreas): This is partially redundant to the renderer texture
    retained_img: Option<RetainedImage>,

    /// For rendering with re_renderer.
    /// `None` if the tensor was not a valid image.
    texture_handle: Option<GpuTexture2DHandle>,

    /// For easily zooming into it in the UI
    /// `None` if the tensor was not a valid image.
    dynamic_img: Option<DynamicImage>,

    /// Total memory used by this image.
    memory_used: u64,

    /// When [`ImageCache::generation`] was we last used?
    last_use_generation: u64,
}

impl CachedImage {
    fn from_tensor(
        render_ctx: &mut RenderContext,
        debug_name: String,
        tensor: &dyn AsDynamicImage,
        annotations: &Arc<Annotations>,
    ) -> Self {
        crate::profile_function!();

        match tensor.as_dynamic_image(annotations) {
            Ok(dynamic_img) => {
                Self::from_dynamic_image(render_ctx, debug_name, dynamic_img, tensor.meaning())
            }
            Err(err) => {
                re_log::warn!("Bad image {debug_name:?}: {}", re_error::format(&err));

                Self {
                    retained_img: None,
                    texture_handle: None,
                    dynamic_img: None,
                    memory_used: 0,
                    last_use_generation: 0,
                }
            }
        }
    }

    fn from_dynamic_image(
        render_ctx: &mut RenderContext,
        debug_name: String,
        dynamic_img: DynamicImage,
        meaning: TensorDataMeaning,
    ) -> Self {
        crate::profile_function!();

        // TODO(andreas): We should not need to create an egui image and instead pass re_renderer's texture to egui if need to be.
        //                  This will save lots of time, memory and frees us up for more formats and colormapping.
        //                  See also https://github.com/rerun-io/rerun/issues/910
        let egui_color_image = dynamic_image_to_egui_color_image(&dynamic_img, meaning);

        let memory_used = egui_color_image.pixels.len() * std::mem::size_of::<egui::Color32>()
            + dynamic_img.as_bytes().len();

        // TODO(andreas): The renderer should ingest images with less conversion (e.g. keep luma as 8bit texture, don't flip bits on bgra etc.)
        let renderer_texture_handle = render_ctx.texture_manager_2d.create(
            &mut render_ctx.gpu_resources.textures,
            &Texture2DCreationDesc {
                label: debug_name.clone().into(),
                data: bytemuck::cast_slice(&egui_color_image.pixels),
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                width: egui_color_image.width() as u32,
                height: egui_color_image.height() as u32,
            },
        );

        let options = egui::TextureOptions {
            // This is best for low-res depth-images and the like
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Linear,
        };
        let retained_img =
            RetainedImage::from_color_image(debug_name, egui_color_image).with_options(options);

        Self {
            dynamic_img: Some(dynamic_img),
            retained_img: Some(retained_img),
            texture_handle: Some(renderer_texture_handle),
            memory_used: memory_used as u64,
            last_use_generation: 0,
        }
    }
}

//TODO(john) this should live in re_log_types along with annotations-related stuff
pub trait AsDynamicImage: component_types::TensorTrait {
    fn as_dynamic_image(&self, annotations: &Arc<Annotations>) -> anyhow::Result<DynamicImage>;
}

impl AsDynamicImage for ClassicTensor {
    fn as_dynamic_image(&self, _annotations: &Arc<Annotations>) -> anyhow::Result<DynamicImage> {
        anyhow::bail!("ClassicTensor is deprecated")
    }
}

impl AsDynamicImage for component_types::Tensor {
    fn as_dynamic_image(&self, annotations: &Arc<Annotations>) -> anyhow::Result<DynamicImage> {
        use anyhow::Context as _;
        let tensor = self;

        crate::profile_function!(format!(
            "dtype: {}, meaning: {:?}",
            tensor.dtype(),
            tensor.meaning
        ));

        let shape = &tensor.shape();

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
        debug_assert!(
            tensor.is_shaped_like_an_image(),
            "We should make the same checks above, but with actual error messages"
        );

        type Gray16Image = image::ImageBuffer<image::Luma<u16>, Vec<u16>>;
        type Rgb16Image = image::ImageBuffer<image::Rgb<u16>, Vec<u16>>;
        type Rgba16Image = image::ImageBuffer<image::Rgba<u16>, Vec<u16>>;

        use egui::epaint::ecolor::gamma_u8_from_linear_f32;
        use egui::epaint::ecolor::linear_u8_from_linear_f32;

        match (depth, &tensor.data, tensor.meaning) {
            (1, TensorData::U8(buf), TensorDataMeaning::ClassId) => {
                // Apply annotation mapping to raw bytes interpreted as u8
                let color_lookup: Vec<[u8; 4]> = (0..256)
                    .map(|id| {
                        annotations
                            .class_description(Some(ClassId(id)))
                            .annotation_info()
                            .color(None, DefaultColor::TransparentBlack)
                            .to_array()
                    })
                    .collect();
                let color_bytes = buf
                    .iter()
                    .flat_map(|p: &u8| color_lookup[*p as usize])
                    .collect();
                crate::profile_scope!("from_raw");
                image::RgbaImage::from_raw(width, height, color_bytes)
                    .context("Bad RGBA8")
                    .map(DynamicImage::ImageRgba8)
            }
            (1, TensorData::U16(buf), TensorDataMeaning::ClassId) => {
                // Apply annotations mapping to bytes interpreted as u16
                let mut color_lookup: ahash::HashMap<u16, [u8; 4]> = Default::default();
                let color_bytes = buf
                    .iter()
                    .flat_map(|id: &u16| {
                        *color_lookup.entry(*id).or_insert_with(|| {
                            annotations
                                .class_description(Some(ClassId(*id)))
                                .annotation_info()
                                .color(None, DefaultColor::TransparentBlack)
                                .to_array()
                        })
                    })
                    .collect();
                crate::profile_scope!("from_raw");
                image::RgbaImage::from_raw(width, height, color_bytes)
                    .context("Bad RGBA8")
                    .map(DynamicImage::ImageRgba8)
            }
            (1, TensorData::U8(buf), _) => {
                // TODO(emilk): we should read some meta-data to check if this is luminance or alpha.
                image::GrayImage::from_raw(width, height, buf.clone())
                    .context("Bad Luminance8")
                    .map(DynamicImage::ImageLuma8)
            }
            (1, TensorData::U16(buf), _) => {
                // TODO(emilk): we should read some meta-data to check if this is luminance or alpha.
                Gray16Image::from_raw(width, height, buf.to_vec())
                    .context("Bad Luminance16")
                    .map(DynamicImage::ImageLuma16)
            }
            (1, TensorData::F32(buf), TensorDataMeaning::Depth) => {
                if buf.is_empty() {
                    Ok(DynamicImage::ImageLuma16(Gray16Image::default()))
                } else {
                    // Convert to u16 so we can put them in an image.
                    // TODO(emilk): Eventually we want a renderer that can show f32 images natively.
                    // One big downside of the approach below is that if we have two depth images
                    // in the same range, they cannot be visually compared with each other,
                    // because their individual max-depths will be scaled to 65535.

                    let mut min = f32::INFINITY;
                    let mut max = f32::NEG_INFINITY;
                    for float in buf.iter() {
                        min = min.min(*float);
                        max = max.max(*float);
                    }

                    anyhow::ensure!(
                        min.is_finite() && max.is_finite(),
                        "Depth image had non-finite values"
                    );

                    if min == max {
                        // Uniform image. We can't remap it to a 0-1 range, so do whatever:
                        let ints = buf.iter().map(|&float| float as u16).collect();
                        Gray16Image::from_raw(width, height, ints)
                            .context("Bad Luminance16")
                            .map(DynamicImage::ImageLuma16)
                    } else {
                        let ints = buf
                            .iter()
                            .map(|&float| egui::remap(float, min..=max, 0.0..=65535.0) as u16)
                            .collect();

                        Gray16Image::from_raw(width, height, ints)
                            .context("Bad Luminance16")
                            .map(DynamicImage::ImageLuma16)
                    }
                }
            }
            (1, TensorData::F32(buf), _) => {
                let l = buf.as_slice();
                let colors: Vec<u8> = l.iter().copied().map(linear_u8_from_linear_f32).collect();
                image::GrayImage::from_raw(width, height, colors)
                    .context("Bad Luminance f32")
                    .map(DynamicImage::ImageLuma8)
            }
            (3, TensorData::U8(buf), _) => image::RgbImage::from_raw(width, height, buf.clone())
                .context("Bad RGB8")
                .map(DynamicImage::ImageRgb8),
            (3, TensorData::U16(buf), _) => Rgb16Image::from_raw(width, height, buf.to_vec())
                .context("Bad RGB16 image")
                .map(DynamicImage::ImageRgb16),
            (3, TensorData::F32(buf), _) => {
                let rgb: &[[f32; 3]] = bytemuck::cast_slice(buf.as_slice());
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

            (4, TensorData::U8(buf), _) => image::RgbaImage::from_raw(width, height, buf.clone())
                .context("Bad RGBA8")
                .map(DynamicImage::ImageRgba8),
            (4, TensorData::U16(buf), _) => Rgba16Image::from_raw(width, height, buf.to_vec())
                .context("Bad RGBA16 image")
                .map(DynamicImage::ImageRgba16),
            (4, TensorData::F32(buf), _) => {
                let rgba: &[[f32; 4]] = bytemuck::cast_slice(buf.as_slice());
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
            (depth, TensorData::JPEG(buf), _) => {
                use image::io::Reader as ImageReader;
                let mut reader = ImageReader::new(std::io::Cursor::new(buf));
                reader.set_format(image::ImageFormat::Jpeg);
                // TODO(emilk): handle grayscale JPEG:s (depth == 1)
                let img = {
                    crate::profile_scope!("decode_jpeg");
                    reader.decode()?
                }
                .into_rgb8();

                if depth != 3 || img.width() != width || img.height() != height {
                    anyhow::bail!(
                        "Tensor shape ({shape:?}) did not match jpeg dimensions ({}x{})",
                        img.width(),
                        img.height()
                    )
                }

                Ok(DynamicImage::ImageRgb8(img))
            }

            (_depth, dtype, meaning @ TensorDataMeaning::ClassId) => {
                anyhow::bail!(
                    "Shape={shape:?} and dtype={dtype:?} is incompatible with meaning={meaning:?}"
                )
            }

            (_depth, dtype, _) => {
                anyhow::bail!("Don't know how to turn a tensor of shape={shape:?} and dtype={dtype:?} into an image")
            }
        }
    }
}

fn dynamic_image_to_egui_color_image(
    dynamic_image: &DynamicImage,
    meaning: TensorDataMeaning,
) -> ColorImage {
    crate::profile_function!();

    match (dynamic_image, meaning) {
        // TODO(#910): Color maps shouldn't be hardcoded.
        (DynamicImage::ImageLuma16(gray), TensorDataMeaning::Depth) => ColorImage {
            size: [gray.width() as _, gray.height() as _],
            pixels: gray
                .pixels()
                .map(|pixel| {
                    crate::misc::color_map::turbo_color_map((pixel[0] as f32) / (u16::MAX as f32))
                })
                .collect(),
        },
        (DynamicImage::ImageLuma8(gray), _) => ColorImage {
            size: [gray.width() as _, gray.height() as _],
            pixels: gray
                .pixels()
                .map(|pixel| Color32::from_gray(pixel[0]))
                .collect(),
        },
        (DynamicImage::ImageLuma16(gray), _) => ColorImage {
            size: [gray.width() as _, gray.height() as _],
            pixels: gray
                .pixels()
                .map(|pixel| Color32::from_gray((pixel[0] / 256) as u8))
                .collect(),
        },
        (DynamicImage::ImageRgb8(rgb), _) => ColorImage {
            size: [rgb.width() as _, rgb.height() as _],
            pixels: rgb
                .pixels()
                .map(|rgb| Color32::from_rgb(rgb[0], rgb[1], rgb[2]))
                .collect(),
        },
        (DynamicImage::ImageRgba8(rgba), _) => ColorImage {
            size: [rgba.width() as _, rgba.height() as _],
            pixels: rgba
                .pixels()
                .map(|rgba| Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]))
                .collect(),
        },
        _ => dynamic_image_to_egui_color_image(
            &DynamicImage::ImageRgba8(dynamic_image.to_rgba8()),
            meaning,
        ),
    }
}
