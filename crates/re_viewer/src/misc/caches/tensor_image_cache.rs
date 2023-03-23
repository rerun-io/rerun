use std::{hash::Hash, sync::Arc};

use egui::{Color32, ColorImage};
use egui_extras::RetainedImage;
use image::DynamicImage;
use re_log_types::{
    component_types::{self, ClassId, Tensor, TensorData, TensorDataMeaning, TensorTrait},
    MsgId,
};
use re_renderer::{
    resource_managers::{GpuTexture2DHandle, Texture2DCreationDesc},
    RenderContext,
};

use crate::{
    misc::caches::TensorStats,
    ui::{Annotations, DefaultColor, MISSING_ANNOTATIONS},
};

// ---

/// The [`ColoredTensorView`] is a wrapper on top of [`Tensor`]
///
/// It consolidates the common operations of going from the raw tensor storage
/// into an object that can be more natively displayed as an Image.
///
/// In the case of images that leverage a `ColorMapping` this includes conversion from
/// the native Tensor type A -> Color32.
pub struct ColoredTensorView<'store, 'cache> {
    /// Key used to retrieve this cached view
    key: ImageCacheKey,

    /// Borrowed tensor from the data store
    pub tensor: &'store Tensor,

    /// Annotations used to create the view
    pub annotations: &'store Arc<Annotations>,

    /// Image with annotations applied and converted to Srgb
    pub colored_image: Option<&'cache ColorImage>,

    // For egui
    // TODO(jleibs): This should go away. See [#506](https://github.com/rerun-io/rerun/issues/506)
    pub retained_image: Option<&'cache RetainedImage>,
}

impl<'store, 'cache> ColoredTensorView<'store, 'cache> {
    /// Try to get a [`GpuTexture2DHandle`] for the cached [`Tensor`].
    ///
    /// Will return None if a valid [`ColorImage`] could not be derived from the [`Tensor`].
    pub fn texture_handle(&self, render_ctx: &mut RenderContext) -> Option<GpuTexture2DHandle> {
        crate::profile_function!();
        self.colored_image.map(|i| {
            let texture_key = self.key.hash64();

            let debug_name = format!("tensor {:?}", self.tensor.shape());
            // TODO(andreas): The renderer should ingest images with less conversion (e.g. keep luma as 8bit texture, don't flip bits on bgra etc.)
            render_ctx.texture_manager_2d.get_or_create(
                texture_key,
                &mut render_ctx.gpu_resources.textures,
                &Texture2DCreationDesc {
                    label: debug_name.into(),
                    data: bytemuck::cast_slice(&i.pixels),
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    width: i.width() as u32,
                    height: i.height() as u32,
                },
            )
        })
    }

    /// Try to get a [`DynamicImage`] for the the cached [`Tensor`].
    ///
    /// Note: this is a `DynamicImage` created from the cached [`ColorImage`], not from the
    /// raw [`Tensor`], as such it will always be a [`DynamicImage::ImageRgba8`].
    ///
    /// Will return None if a valid [`ColorImage`] could not be derived from the [`Tensor`].
    pub fn dynamic_img(&self) -> Option<DynamicImage> {
        crate::profile_function!();
        self.colored_image.and_then(|i| {
            let bytes: &[u8] = bytemuck::cast_slice(&i.pixels);
            image::RgbaImage::from_raw(i.width() as _, i.height() as _, bytes.into())
                .map(DynamicImage::ImageRgba8)
        })
    }
}

// Use this for the cache index so that we don't cache across
// changes to the annotations
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ImageCacheKey {
    tensor_id: component_types::TensorId,
    annotation_msg_id: MsgId,
}

impl ImageCacheKey {
    fn hash64(&self) -> u64 {
        let msg_hash = self.tensor_id.0.as_u128() as u64;
        let annotation_hash = (self.annotation_msg_id.as_u128() >> 1) as u64;
        msg_hash ^ annotation_hash
    }
}

impl nohash_hasher::IsEnabled for ImageCacheKey {}

// required for [`nohash_hasher`].
#[allow(clippy::derive_hash_xor_eq)]
impl Hash for ImageCacheKey {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash64());
    }
}

#[derive(Default)]
pub struct ImageCache {
    images: nohash_hasher::IntMap<ImageCacheKey, CachedImage>,
    memory_used: u64,
    generation: u64,
}

impl ImageCache {
    pub(crate) fn get_colormapped_view<'store, 'cache>(
        &'cache mut self,
        tensor: &'store Tensor,
        annotations: &'store Arc<Annotations>,
    ) -> ColoredTensorView<'store, 'cache> {
        let key = ImageCacheKey {
            tensor_id: tensor.id(),
            annotation_msg_id: annotations.msg_id,
        };
        let ci = self.images.entry(key).or_insert_with(|| {
            let debug_name = format!("tensor {:?}", tensor.shape());
            let ci = CachedImage::from_tensor(&debug_name, tensor, annotations);
            self.memory_used += ci.memory_used;
            ci
        });
        ci.last_use_generation = self.generation;

        ColoredTensorView::<'store, '_> {
            key,
            tensor,
            annotations,
            colored_image: ci.colored_image.as_ref(),
            retained_image: ci.retained_image.as_ref(),
        }
    }

    pub(crate) fn get_view<'store, 'cache>(
        &'cache mut self,
        tensor: &'store Tensor,
    ) -> ColoredTensorView<'store, 'cache> {
        self.get_colormapped_view(tensor, &MISSING_ANNOTATIONS)
    }

    /// Call once per frame to (potentially) flush the cache.
    pub fn begin_frame(&mut self, max_memory_use: u64) {
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
    /// For uploading to GPU
    colored_image: Option<ColorImage>,

    // For egui
    // TODO(jleibs): This should go away. See [#506](https://github.com/rerun-io/rerun/issues/506)
    retained_image: Option<RetainedImage>,

    /// Total memory used by this image.
    memory_used: u64,

    /// When [`ImageCache::generation`] was we last used?
    last_use_generation: u64,
}

impl CachedImage {
    fn from_tensor(debug_name: &str, tensor: &Tensor, annotations: &Arc<Annotations>) -> Self {
        crate::profile_function!();

        match apply_color_map(tensor, annotations) {
            Ok(colored_image) => {
                let memory_used = colored_image.pixels.len() * std::mem::size_of::<egui::Color32>();

                let retained_image = {
                    crate::profile_scope!("retained_image");
                    let options = egui::TextureOptions {
                        // This is best for low-res depth-images and the like
                        magnification: egui::TextureFilter::Nearest,
                        minification: egui::TextureFilter::Linear,
                    };
                    RetainedImage::from_color_image(debug_name, colored_image.clone())
                        .with_options(options)
                };

                Self {
                    colored_image: Some(colored_image),
                    retained_image: Some(retained_image),
                    memory_used: memory_used as u64,
                    last_use_generation: 0,
                }
            }
            Err(err) => {
                re_log::warn!("Bad image {debug_name:?}: {}", re_error::format(&err));

                Self {
                    colored_image: None,
                    retained_image: None,
                    memory_used: 0,
                    last_use_generation: 0,
                }
            }
        }
    }
}

fn apply_color_map(tensor: &Tensor, annotations: &Arc<Annotations>) -> anyhow::Result<ColorImage> {
    match tensor.meaning {
        TensorDataMeaning::Unknown => color_tensor_as_color_image(tensor),
        TensorDataMeaning::ClassId => class_id_tensor_as_color_image(tensor, annotations),
        TensorDataMeaning::Depth => depth_tensor_as_color_image(tensor),
    }
}

fn height_width_depth(tensor: &Tensor) -> anyhow::Result<[u32; 3]> {
    use anyhow::Context as _;

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

    Ok([height, width, depth as u32])
}

fn color_tensor_as_color_image(tensor: &Tensor) -> anyhow::Result<ColorImage> {
    crate::profile_function!(format!(
        "dtype: {}, shape: {:?}",
        tensor.dtype(),
        tensor.shape()
    ));

    let [height, width, depth] = height_width_depth(tensor)?;

    use egui::epaint::ecolor::{gamma_u8_from_linear_f32, linear_u8_from_linear_f32};

    let size = [width as _, height as _];

    match (depth, &tensor.data) {
        (1, TensorData::U8(buf)) => {
            // TODO(emilk): we should read some meta-data to check if this is luminance or alpha.
            let pixels = buf
                .0
                .iter()
                .map(|pixel| Color32::from_gray(*pixel))
                .collect();
            Ok(ColorImage { size, pixels })
        }
        (1, TensorData::U16(buf)) => {
            // TODO(emilk): we should read some meta-data to check if this is luminance or alpha.
            let pixels = buf
                .iter()
                .map(|pixel| Color32::from_gray((*pixel / 256) as u8))
                .collect();

            Ok(ColorImage { size, pixels })
        }
        (1, TensorData::F32(buf)) => {
            let pixels = buf
                .iter()
                .map(|pixel| Color32::from_gray(linear_u8_from_linear_f32(*pixel)))
                .collect();

            Ok(ColorImage { size, pixels })
        }
        (3, TensorData::U8(buf)) => Ok(ColorImage::from_rgb(size, buf.0.as_slice())),
        (3, TensorData::U16(buf)) => {
            let u8_buf: Vec<u8> = buf.iter().map(|pixel| (*pixel / 256) as u8).collect();

            Ok(ColorImage::from_rgb(size, &u8_buf))
        }
        (3, TensorData::F32(buf)) => {
            let rgb: &[[f32; 3]] = bytemuck::cast_slice(buf.as_slice());
            let pixels: Vec<Color32> = rgb
                .iter()
                .map(|&[r, g, b]| {
                    let r = gamma_u8_from_linear_f32(r);
                    let g = gamma_u8_from_linear_f32(g);
                    let b = gamma_u8_from_linear_f32(b);
                    Color32::from_rgb(r, g, b)
                })
                .collect();

            Ok(ColorImage { size, pixels })
        }

        (4, TensorData::U8(buf)) => Ok(ColorImage::from_rgba_unmultiplied(size, buf.0.as_slice())),
        (4, TensorData::U16(buf)) => {
            let u8_buf: Vec<u8> = buf.iter().map(|pixel| (*pixel / 256) as u8).collect();

            Ok(ColorImage::from_rgba_unmultiplied(size, &u8_buf))
        }
        (4, TensorData::F32(buf)) => {
            let rgba: &[[f32; 4]] = bytemuck::cast_slice(buf.as_slice());
            let pixels: Vec<Color32> = rgba
                .iter()
                .map(|&[r, g, b, a]| {
                    let r = gamma_u8_from_linear_f32(r);
                    let g = gamma_u8_from_linear_f32(g);
                    let b = gamma_u8_from_linear_f32(b);
                    let a = linear_u8_from_linear_f32(a);
                    Color32::from_rgba_unmultiplied(r, g, b, a)
                })
                .collect();

            Ok(ColorImage { size, pixels })
        }

        (_depth, dtype) => {
            anyhow::bail!("Don't know how to turn a tensor of shape={:?} and dtype={dtype:?} into a color image", tensor.shape)
        }
    }
}

fn class_id_tensor_as_color_image(
    tensor: &Tensor,
    annotations: &Annotations,
) -> anyhow::Result<ColorImage> {
    crate::profile_function!(format!(
        "dtype: {}, shape: {:?}",
        tensor.dtype(),
        tensor.shape()
    ));

    let [height, width, depth] = height_width_depth(tensor)?;
    anyhow::ensure!(
        depth == 1,
        "Cannot apply annotations to tensor of shape {:?}",
        tensor.shape
    );
    let size = [width as _, height as _];

    match &tensor.data {
        TensorData::U8(buf) => {
            // Apply annotation mapping to raw bytes interpreted as u8
            let color_lookup: Vec<Color32> = (0..256)
                .map(|id| {
                    annotations
                        .class_description(Some(ClassId(id)))
                        .annotation_info()
                        .color(None, DefaultColor::TransparentBlack)
                })
                .collect();
            let pixels: Vec<Color32> = buf
                .0
                .iter()
                .map(|p: &u8| color_lookup[*p as usize])
                .collect();
            crate::profile_scope!("from_raw");
            Ok(ColorImage { size, pixels })
        }
        TensorData::U16(buf) => {
            // Apply annotations mapping to bytes interpreted as u16
            let mut color_lookup: ahash::HashMap<u16, Color32> = Default::default();
            let pixels = buf
                .iter()
                .map(|id: &u16| {
                    *color_lookup.entry(*id).or_insert_with(|| {
                        annotations
                            .class_description(Some(ClassId(*id)))
                            .annotation_info()
                            .color(None, DefaultColor::TransparentBlack)
                    })
                })
                .collect();
            crate::profile_scope!("from_raw");
            Ok(ColorImage { size, pixels })
        }
        _ => {
            anyhow::bail!(
                "Cannot apply annotations to tensor of dtype {}",
                tensor.dtype()
            )
        }
    }
}

fn depth_tensor_as_color_image(tensor: &Tensor) -> anyhow::Result<ColorImage> {
    if tensor.data.is_empty() {
        return Ok(ColorImage::default());
    }

    // This function applies color mapping to a depth image.
    // We are planning on moving this to the GPU: https://github.com/rerun-io/rerun/issues/1612
    // One big downside of the approach below is that if we have two depth images
    // in the same range, they cannot be visually compared with each other,
    // because their individual max-depths will be scaled to 65535.

    crate::profile_function!(format!(
        "dtype: {}, shape: {:?}",
        tensor.dtype(),
        tensor.shape()
    ));

    let [height, width, depth] = height_width_depth(tensor)?;
    anyhow::ensure!(depth == 1, "Depth tensor of shape {:?}", tensor.shape);
    let size = [width as _, height as _];

    let range = TensorStats::new(tensor).range.ok_or(anyhow::anyhow!(
        "Depth image had no range!? Was this compressed?"
    ))?;

    let (mut min, mut max) = range;

    anyhow::ensure!(
        min.is_finite() && max.is_finite(),
        "Depth image had non-finite values"
    );

    if min == max {
        // Uniform image. We can't remap it to a 0-1 range, so do whatever:
        min = 0.0;
        max = if tensor.dtype().is_float() {
            1.0
        } else {
            tensor.dtype().max_value()
        };
    }

    let colormap = |value: f64| {
        let t = egui::remap(value, min..=max, 0.0..=1.0) as f32;
        let [r, g, b, _] = re_renderer::colormap_turbo_srgb(t);
        egui::Color32::from_rgb(r, g, b)
    };

    match &tensor.data {
        TensorData::U8(buf) => {
            let pixels = buf.iter().map(|&value| colormap(value as _)).collect();
            Ok(ColorImage { size, pixels })
        }
        TensorData::U16(buf) => {
            let pixels = buf.iter().map(|&value| colormap(value as _)).collect();
            Ok(ColorImage { size, pixels })
        }
        TensorData::U32(buf) => {
            let pixels = buf.iter().map(|&value| colormap(value as _)).collect();
            Ok(ColorImage { size, pixels })
        }
        TensorData::U64(buf) => {
            let pixels = buf.iter().map(|&value| colormap(value as _)).collect();
            Ok(ColorImage { size, pixels })
        }

        TensorData::I8(buf) => {
            let pixels = buf.iter().map(|&value| colormap(value as _)).collect();
            Ok(ColorImage { size, pixels })
        }
        TensorData::I16(buf) => {
            let pixels = buf.iter().map(|&value| colormap(value as _)).collect();
            Ok(ColorImage { size, pixels })
        }
        TensorData::I32(buf) => {
            let pixels = buf.iter().map(|&value| colormap(value as _)).collect();
            Ok(ColorImage { size, pixels })
        }
        TensorData::I64(buf) => {
            let pixels = buf.iter().map(|&value| colormap(value as _)).collect();
            Ok(ColorImage { size, pixels })
        }

        TensorData::F32(buf) => {
            let pixels = buf.iter().map(|&value| colormap(value as _)).collect();
            Ok(ColorImage { size, pixels })
        }
        TensorData::F64(buf) => {
            let pixels = buf.iter().map(|&value| colormap(value)).collect();
            Ok(ColorImage { size, pixels })
        }

        TensorData::JPEG(_) => {
            anyhow::bail!("Cannot apply colormap to JPEG image")
        }
    }
}
