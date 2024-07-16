//! Upload images to [`re_renderer`].

use std::borrow::Cow;

use anyhow::Context as _;
use egui::util::hash;
use wgpu::TextureFormat;

use re_renderer::{
    pad_rgb_to_rgba,
    renderer::{ColorMapper, ColormappedTexture},
    resource_managers::Texture2DCreationDesc,
    RenderContext,
};
use re_types::components::{ClassId, ColorModel, Colormap};
use re_types::{components::ElementType, tensor_data::TensorDataMeaning};

use crate::{
    gpu_bridge::colormap::colormap_to_re_renderer, Annotations, ImageComponents, TensorStats,
};

use super::get_or_create_texture;

// ----------------------------------------------------------------------------

/// Returns a texture key for a given row id & usage.
///
/// Several textures may be created from the same row.
/// This makes sure that they all get different keys!
fn generate_texture_key(image: &ImageComponents, meaning: TensorDataMeaning) -> u64 {
    // We need to inclde anything that, if changes, should result in a new texture being uploaded.
    let ImageComponents {
        row_id,
        blob: _, // from `row_id`
        resolution,
        element_type,
        color_model,
        colormap,
    } = image;

    hash((
        row_id,
        resolution,
        element_type,
        color_model,
        colormap,
        meaning,
    ))
}

pub fn image_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    image: &ImageComponents,
    meaning: TensorDataMeaning,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
) -> anyhow::Result<ColormappedTexture> {
    re_tracing::profile_function!(format!(
        "meaning: {:?}, resolution: {:?}, element_type: {:?}",
        meaning, image.resolution, image.element_type,
    ));

    let texture_key = generate_texture_key(image, meaning);

    match meaning {
        TensorDataMeaning::Unknown => {
            color_image_to_gpu(render_ctx, debug_name, texture_key, image, tensor_stats)
        }
        TensorDataMeaning::Depth => {
            depth_image_to_gpu(render_ctx, debug_name, texture_key, image, tensor_stats)
        }
        TensorDataMeaning::ClassId => segmentation_image_to_gpu(
            render_ctx,
            debug_name,
            texture_key,
            image,
            tensor_stats,
            annotations,
        ),
    }
}

fn color_image_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    texture_key: u64,
    image: &ImageComponents,
    tensor_stats: &TensorStats,
) -> Result<ColormappedTexture, anyhow::Error> {
    re_tracing::profile_function!();

    let element_type = image.element_type;
    let color_model = image.color_model.unwrap_or_default();

    let texture_handle = get_or_create_texture(render_ctx, texture_key, || {
        // Profile creation of the texture, but not cache hits (those take close to no time, which would just add profiler overhead).
        re_tracing::profile_function!();

        let (data, format) = match (color_model, element_type) {
            // Normalize sRGB(A) textures to 0-1 range, and let the GPU premultiply alpha.
            // Why? Because premul must happen _before_ sRGB decode, so we can't
            // use a "Srgb-aware" texture like `Rgba8UnormSrgb` for RGBA.
            (ColorModel::Rgb, ElementType::U8) => (
                pad_rgb_to_rgba(&image.blob, u8::MAX).into(),
                TextureFormat::Rgba8Unorm,
            ),

            (ColorModel::Rgba, ElementType::U8) => {
                (cast_slice_to_cow(&image.blob), TextureFormat::Rgba8Unorm)
            }

            _ => {
                // Fallback to general case:
                return general_texture_creation_desc_from_image(debug_name, image);
            }
        };

        Texture2DCreationDesc {
            label: debug_name.into(),
            data,
            format,
            width: image.width(),
            height: image.height(),
        }
    })
    .map_err(|err| anyhow::anyhow!("{err}"))?;

    let texture_format = texture_handle.format();

    // Special casing for normalized textures used above:
    let range = if matches!(
        texture_format,
        TextureFormat::R8Unorm | TextureFormat::Rgba8Unorm
    ) {
        [0.0, 1.0]
    } else if texture_format == TextureFormat::R8Snorm {
        [-1.0, 1.0]
    } else {
        // TODO(#2341): The range should be determined by a `DataRange` component. In absence this, heuristics apply.
        super::image_data_range_heuristic(tensor_stats, element_type)?
    };

    // TODO(emilk): let the user specify the color space.
    let decode_srgb = texture_format == TextureFormat::Rgba8Unorm
        || super::image_decode_srgb_gamma_heuristic(tensor_stats, element_type, color_model)?;

    let color_mapper = if texture_format.components() == 1 {
        // TODO(andreas): support colormap property
        if decode_srgb {
            // Leave grayscale images unmolested - don't apply a colormap to them.
            ColorMapper::OffGrayscale
        } else {
            // This is something like a uint16 image, or a float image
            // with a range outside of 0-255 (see image_decode_srgb_gamma_heuristic).
            // `tensor_data_range_heuristic` will make sure we map this to a 0-1
            // range, and then we apply a gray colormap to it.
            ColorMapper::Function(re_renderer::Colormap::Grayscale)
        }
    } else {
        ColorMapper::OffRGB
    };

    // TODO(wumpf): There should be a way to specify whether a texture uses pre-multiplied alpha or not.
    let multiply_rgb_with_alpha = match color_model {
        ColorModel::L | ColorModel::Rgb => false, // No alpha

        ColorModel::Rgba => true, // Assume that the texture is not pre-multiplied
    };
    let gamma = 1.0;

    re_log::trace_once!(
        "color_tensor_to_gpu {debug_name:?}, range: {range:?}, decode_srgb: {decode_srgb:?}, multiply_rgb_with_alpha: {multiply_rgb_with_alpha:?}, gamma: {gamma:?}, color_mapper: {color_mapper:?}",
    );

    Ok(ColormappedTexture {
        texture: texture_handle,
        range,
        decode_srgb,
        multiply_rgb_with_alpha,
        gamma,
        color_mapper,
        shader_decoding: None,
    })
}

fn depth_image_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    texture_key: u64,
    image: &ImageComponents,
    tensor_stats: &TensorStats,
) -> Result<ColormappedTexture, anyhow::Error> {
    re_tracing::profile_function!();

    let range = data_range(tensor_stats, image.element_type);

    let texture = get_or_create_texture(render_ctx, texture_key, || {
        general_texture_creation_desc_from_image(debug_name, image)
    })
    .map_err(|err| anyhow::anyhow!("Failed to create depth texture: {err}"))?;

    Ok(ColormappedTexture {
        texture,
        range,
        decode_srgb: false,
        multiply_rgb_with_alpha: false,
        gamma: 1.0,
        color_mapper: ColorMapper::Function(colormap_to_re_renderer(
            image.colormap.unwrap_or(Colormap::Turbo),
        )),
        shader_decoding: None,
    })
}

fn segmentation_image_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    texture_key: u64,
    image: &ImageComponents,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
) -> Result<ColormappedTexture, anyhow::Error> {
    re_tracing::profile_function!();

    let colormap_key = hash(annotations.row_id());

    let (_, mut max) = tensor_stats
        .range
        .ok_or_else(|| anyhow::anyhow!("compressed_tensor!?"))?;

    // We only support u8 and u16 class ids.
    // Any values greater than this will be unmapped in the segmentation image.
    max = max.min(65535.0);

    // We pack the colormap into a 2D texture so we don't go over the max texture size.
    // We only support u8 and u16 class ids, so 256^2 is the biggest texture we need.
    let num_colors = (max + 1.0) as usize;
    let colormap_width = 256;
    let colormap_height = (num_colors + colormap_width - 1) / colormap_width;

    let colormap_texture_handle = get_or_create_texture(render_ctx, colormap_key, || {
        let data: Vec<u8> = (0..(colormap_width * colormap_height))
            .flat_map(|id| {
                let color = annotations
                    .resolved_class_description(Some(ClassId::from(id as u16)))
                    .annotation_info()
                    .color()
                    .unwrap_or(re_renderer::Color32::TRANSPARENT);
                color.to_array() // premultiplied!
            })
            .collect();

        Texture2DCreationDesc {
            label: "class_id_colormap".into(),
            data: data.into(),
            format: TextureFormat::Rgba8UnormSrgb,
            width: colormap_width as u32,
            height: colormap_height as u32,
        }
    })
    .context("Failed to create class_id_colormap.")?;

    let main_texture_handle = get_or_create_texture(render_ctx, texture_key, || {
        general_texture_creation_desc_from_image(debug_name, image)
    })
    .map_err(|err| anyhow::anyhow!("{err}"))?;

    Ok(ColormappedTexture {
        texture: main_texture_handle,
        range: [0.0, (colormap_width * colormap_height) as f32],
        decode_srgb: false, // Setting this to true would affect the class ids, not the color they resolve to.
        multiply_rgb_with_alpha: false, // already premultiplied!
        gamma: 1.0,
        color_mapper: ColorMapper::Texture(colormap_texture_handle),
        shader_decoding: None,
    })
}

fn data_range(tensor_stats: &TensorStats, element_type: ElementType) -> [f32; 2] {
    let default_min = 0.0;
    let default_max = if element_type.is_float() {
        1.0
    } else {
        element_type.max_value()
    };

    let range = tensor_stats
        .finite_range
        .unwrap_or((default_min, default_max));
    let (mut min, mut max) = range;

    if !min.is_finite() {
        min = default_min;
    }
    if !max.is_finite() {
        max = default_max;
    }

    if max <= min {
        min = default_min;
        max = default_max;
    }

    [min as f32, max as f32]
}

/// Uploads the image to a texture in a format that closely resembled the input.
/// Uses no `Unorm/Snorm` formats.
fn general_texture_creation_desc_from_image<'a>(
    debug_name: &str,
    image: &'a ImageComponents,
) -> Texture2DCreationDesc<'a> {
    re_tracing::profile_function!();

    let [width, height] = image.resolution;

    let buf: &[u8] = image.blob.as_ref();

    let color_model = image.color_model.unwrap_or(ColorModel::L);

    let (data, format) = match color_model {
        ColorModel::L => {
            match image.element_type {
                ElementType::U8 => (Cow::Borrowed(buf), TextureFormat::R8Uint),
                ElementType::U16 => (Cow::Borrowed(buf), TextureFormat::R16Uint),
                ElementType::U32 => (Cow::Borrowed(buf), TextureFormat::R32Uint),
                ElementType::U64 => (
                    // wgpu doesn't support u64 textures
                    narrow_u64_to_f32s(&image.to_slice()),
                    TextureFormat::R32Float,
                ),

                ElementType::I8 => (Cow::Borrowed(buf), TextureFormat::R8Sint),
                ElementType::I16 => (Cow::Borrowed(buf), TextureFormat::R16Sint),
                ElementType::I32 => (Cow::Borrowed(buf), TextureFormat::R32Sint),
                ElementType::I64 => (
                    // wgpu doesn't support i64 textures
                    narrow_i64_to_f32s(&image.to_slice()),
                    TextureFormat::R32Float,
                ),

                ElementType::F16 => (Cow::Borrowed(buf), TextureFormat::R16Float),
                ElementType::F32 => (Cow::Borrowed(buf), TextureFormat::R32Float),
                ElementType::F64 => (
                    // wgpu doesn't support f64 textures
                    narrow_f64_to_f32s(&image.to_slice()),
                    TextureFormat::R32Float,
                ),
            }
        }

        ColorModel::Rgb => {
            // There are no 3-channel textures in wgpu, so we need to pad to 4 channels.
            // What should we pad with? It depends on whether or not the shader interprets these as alpha.
            // To be safe, we pad with the MAX value of integers, and with 1.0 for floats.
            // TODO(emilk): tell the shader to ignore the alpha channel instead!

            match image.element_type {
                ElementType::U8 => (
                    pad_rgb_to_rgba(buf, u8::MAX).into(),
                    TextureFormat::Rgba8Uint,
                ),
                ElementType::U16 => (
                    pad_and_cast(&image.to_slice(), u16::MAX),
                    TextureFormat::Rgba16Uint,
                ),
                ElementType::U32 => (
                    pad_and_cast(&image.to_slice(), u32::MAX),
                    TextureFormat::Rgba32Uint,
                ),
                ElementType::U64 => (
                    pad_and_narrow_and_cast(&image.to_slice(), 1.0, |x: u64| x as f32),
                    TextureFormat::Rgba32Float,
                ),

                ElementType::I8 => (
                    pad_and_cast(&image.to_slice(), i8::MAX),
                    TextureFormat::Rgba8Sint,
                ),
                ElementType::I16 => (
                    pad_and_cast(&image.to_slice(), i16::MAX),
                    TextureFormat::Rgba16Sint,
                ),
                ElementType::I32 => (
                    pad_and_cast(&image.to_slice(), i32::MAX),
                    TextureFormat::Rgba32Sint,
                ),
                ElementType::I64 => (
                    pad_and_narrow_and_cast(&image.to_slice(), 1.0, |x: i64| x as f32),
                    TextureFormat::Rgba32Float,
                ),

                ElementType::F16 => (
                    pad_and_cast(
                        &image.to_slice(),
                        re_log_types::external::arrow2::types::f16::from_f32(1.0),
                    ),
                    TextureFormat::Rgba16Float,
                ),
                ElementType::F32 => (
                    pad_and_cast(&image.to_slice(), 1.0),
                    TextureFormat::Rgba32Float,
                ),
                ElementType::F64 => (
                    pad_and_narrow_and_cast(&image.to_slice(), 1.0, |x: f64| x as f32),
                    TextureFormat::Rgba32Float,
                ),
            }
        }

        ColorModel::Rgba => {
            // TODO(emilk): premultiply alpha, or tell the shader to assume unmultiplied alpha

            match image.element_type {
                ElementType::U8 => (Cow::Borrowed(buf), TextureFormat::Rgba8Uint),
                ElementType::U16 => (Cow::Borrowed(buf), TextureFormat::Rgba16Uint),
                ElementType::U32 => (Cow::Borrowed(buf), TextureFormat::Rgba32Uint),
                ElementType::U64 => (
                    // wgpu doesn't support u64 textures
                    narrow_u64_to_f32s(&image.to_slice()),
                    TextureFormat::Rgba32Float,
                ),

                ElementType::I8 => (Cow::Borrowed(buf), TextureFormat::Rgba8Sint),
                ElementType::I16 => (Cow::Borrowed(buf), TextureFormat::Rgba16Sint),
                ElementType::I32 => (Cow::Borrowed(buf), TextureFormat::Rgba32Sint),
                ElementType::I64 => (
                    // wgpu doesn't support i64 textures
                    narrow_i64_to_f32s(&image.to_slice()),
                    TextureFormat::Rgba32Float,
                ),

                ElementType::F16 => (Cow::Borrowed(buf), TextureFormat::Rgba16Float),
                ElementType::F32 => (Cow::Borrowed(buf), TextureFormat::Rgba32Float),
                ElementType::F64 => (
                    // wgpu doesn't support f64 textures
                    narrow_f64_to_f32s(&image.to_slice()),
                    TextureFormat::Rgba32Float,
                ),
            }
        }
    };

    Texture2DCreationDesc {
        label: debug_name.into(),
        data,
        format,
        width,
        height,
    }
}

fn cast_slice_to_cow<From: bytemuck::Pod>(slice: &[From]) -> Cow<'_, [u8]> {
    bytemuck::cast_slice(slice).into()
}

// wgpu doesn't support u64 textures, so we need to narrow to f32:
fn narrow_u64_to_f32s(slice: &[u64]) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();
    let bytes: Vec<u8> = slice
        .iter()
        .flat_map(|&f| (f as f32).to_le_bytes())
        .collect();
    bytes.into()
}

// wgpu doesn't support i64 textures, so we need to narrow to f32:
fn narrow_i64_to_f32s(slice: &[i64]) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();
    let bytes: Vec<u8> = slice
        .iter()
        .flat_map(|&f| (f as f32).to_le_bytes())
        .collect();
    bytes.into()
}

// wgpu doesn't support f64 textures, so we need to narrow to f32:
fn narrow_f64_to_f32s(slice: &[f64]) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();
    let bytes: Vec<u8> = slice
        .iter()
        .flat_map(|&f| (f as f32).to_le_bytes())
        .collect();
    bytes.into()
}

fn pad_and_cast<T: Copy + bytemuck::Pod>(data: &[T], pad: T) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();
    // TODO(emilk): optimize by combining the two steps into one; avoiding one allocation and memcpy
    let padded: Vec<T> = pad_rgb_to_rgba(data, pad);
    let bytes: Vec<u8> = bytemuck::pod_collect_to_vec(&padded);
    bytes.into()
}

fn pad_and_narrow_and_cast<T: Copy + bytemuck::Pod>(
    data: &[T],
    pad: f32,
    narrow: impl Fn(T) -> f32,
) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();

    let floats: Vec<f32> = data
        .chunks_exact(3)
        .flat_map(|chunk| [narrow(chunk[0]), narrow(chunk[1]), narrow(chunk[2]), pad])
        .collect();
    bytemuck::pod_collect_to_vec(&floats).into()
}
