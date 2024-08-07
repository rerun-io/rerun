//! Upload images to [`re_renderer`].

use std::borrow::Cow;

use anyhow::Context as _;
use egui::util::hash;
use wgpu::TextureFormat;

use re_renderer::{
    pad_rgb_to_rgba,
    renderer::{ColorMapper, ColormappedTexture, ShaderDecoding},
    resource_managers::Texture2DCreationDesc,
    RenderContext,
};
use re_types::components::{ClassId, Colormap};
use re_types::datatypes::{ChannelDatatype, ColorModel, ImageFormat, PixelFormat};
use re_types::image::ImageKind;

use crate::{gpu_bridge::colormap::colormap_to_re_renderer, Annotations, ImageInfo, TensorStats};

use super::{get_or_create_texture, RangeError};

// ----------------------------------------------------------------------------

/// Returns a texture key for the given image.
///
/// If the key changes, we upload a new texture.
fn generate_texture_key(image: &ImageInfo) -> u64 {
    // We need to inclde anything that, if changes, should result in a new texture being uploaded.
    let ImageInfo {
        blob_row_id,
        blob: _, // we hash `blob_row_id` instead; much faster!

        format,
        kind,

        colormap: _, // No need to upload new texture when this changes
    } = image;

    hash((blob_row_id, format, kind))
}

pub fn image_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    image: &ImageInfo,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
) -> anyhow::Result<ColormappedTexture> {
    re_tracing::profile_function!();

    let texture_key = generate_texture_key(image);

    match image.kind {
        ImageKind::Color => {
            color_image_to_gpu(render_ctx, debug_name, texture_key, image, tensor_stats)
        }
        ImageKind::Depth => {
            depth_image_to_gpu(render_ctx, debug_name, texture_key, image, tensor_stats)
        }
        ImageKind::Segmentation => segmentation_image_to_gpu(
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
    image: &ImageInfo,
    tensor_stats: &TensorStats,
) -> anyhow::Result<ColormappedTexture> {
    re_tracing::profile_function!();

    let image_format = image.format;

    let texture_handle = get_or_create_texture(render_ctx, texture_key, || {
        texture_creation_desc_from_color_image(image, debug_name)
    })
    .map_err(|err| anyhow::anyhow!("{err}"))?;

    let texture_format = texture_handle.format();

    let shader_decoding = match image_format.pixel_format {
        Some(PixelFormat::NV12) => Some(ShaderDecoding::Nv12),
        Some(PixelFormat::YUY2) => Some(ShaderDecoding::Yuy2),
        None => None,
    };

    // TODO(emilk): let the user specify the color space.
    let decode_srgb = texture_format == TextureFormat::Rgba8Unorm
        || image_decode_srgb_gamma_heuristic(tensor_stats, image_format)?;

    // Special casing for normalized textures used above:
    let range = if matches!(
        texture_format,
        TextureFormat::R8Unorm | TextureFormat::Rgba8Unorm
    ) {
        [0.0, 1.0]
    } else if texture_format == TextureFormat::R8Snorm {
        [-1.0, 1.0]
    } else if let Some(shader_decoding) = shader_decoding {
        match shader_decoding {
            ShaderDecoding::Nv12 | ShaderDecoding::Yuy2 => [0.0, 1.0],
        }
    } else {
        // TODO(#2341): The range should be determined by a `DataRange` component. In absence this, heuristics apply.
        image_data_range_heuristic(tensor_stats, image_format)?
    };

    let color_mapper = if let Some(shader_decoding) = shader_decoding {
        match shader_decoding {
            ShaderDecoding::Nv12 | ShaderDecoding::Yuy2 => ColorMapper::OffRGB,
        }
    } else if texture_format.components() == 1 {
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

    // Assume that the texture has a separate (non-pre-multiplied) alpha.
    // TODO(wumpf): There should be a way to specify whether a texture uses pre-multiplied alpha or not.
    let multiply_rgb_with_alpha = image_format.has_alpha();

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
        shader_decoding,
    })
}

/// Get a valid, finite range for the gpu to use.
fn image_data_range_heuristic(
    tensor_stats: &TensorStats,
    image_format: ImageFormat,
) -> Result<[f32; 2], RangeError> {
    let (min, max) = tensor_stats.finite_range.ok_or(RangeError::MissingRange)?;

    let min = min as f32;
    let max = max as f32;

    // Apply heuristic for ranges that are typically expected depending on the data type and the finite (!) range.
    // (we ignore NaN/Inf values heres, since they are usually there by accident!)
    if image_format.is_float() && 0.0 <= min && max <= 1.0 {
        // Float values that are all between 0 and 1, assume that this is the range.
        Ok([0.0, 1.0])
    } else if 0.0 <= min && max <= 255.0 {
        // If all values are between 0 and 255, assume this is the range.
        // (This is very common, independent of the data type)
        Ok([0.0, 255.0])
    } else if min == max {
        // uniform range. This can explode the colormapping, so let's map all colors to the middle:
        Ok([min - 1.0, max + 1.0])
    } else {
        // Use range as is if nothing matches.
        Ok([min, max])
    }
}

/// Return whether an image should be assumed to be encoded in sRGB color space ("gamma space", no EOTF applied).
fn image_decode_srgb_gamma_heuristic(
    tensor_stats: &TensorStats,
    image_format: ImageFormat,
) -> Result<bool, RangeError> {
    if let Some(pixel_format) = image_format.pixel_format {
        match pixel_format {
            PixelFormat::NV12 | PixelFormat::YUY2 => Ok(true),
        }
    } else {
        let color_model = image_format.color_model();
        let datatype = image_format.datatype();
        match color_model {
            ColorModel::L | ColorModel::RGB | ColorModel::RGBA => {
                let (min, max) = tensor_stats.finite_range.ok_or(RangeError::MissingRange)?;

                #[allow(clippy::if_same_then_else)]
                if 0.0 <= min && max <= 255.0 {
                    // If the range is suspiciously reminding us of a "regular image", assume sRGB.
                    Ok(true)
                } else if datatype.is_float() && 0.0 <= min && max <= 1.0 {
                    // Floating point images between 0 and 1 are often sRGB as well.
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }
}

fn texture_creation_desc_from_color_image<'a>(
    image: &'a ImageInfo,
    debug_name: &'a str,
) -> Texture2DCreationDesc<'a> {
    re_tracing::profile_function!();

    if let Some(pixel_format) = image.format.pixel_format {
        match pixel_format {
            PixelFormat::NV12 => {
                // Decoded in the shader.
                return Texture2DCreationDesc {
                    label: debug_name.into(),
                    data: cast_slice_to_cow(image.blob.as_slice()),
                    format: TextureFormat::R8Uint,
                    width: image.width(),
                    height: image.height() + image.height() / 2, // !
                };
            }

            PixelFormat::YUY2 => {
                // Decoded in the shader.
                return Texture2DCreationDesc {
                    label: debug_name.into(),
                    data: cast_slice_to_cow(image.blob.as_slice()),
                    format: TextureFormat::R8Uint,
                    width: 2 * image.width(), // !
                    height: image.height(),
                };
            }
        }
    } else {
        let color_model = image.format.color_model();
        let datatype = image.format.datatype();
        let (data, format) = match (color_model, datatype) {
            // Normalize sRGB(A) textures to 0-1 range, and let the GPU premultiply alpha.
            // Why? Because premul must happen _before_ sRGB decode, so we can't
            // use a "Srgb-aware" texture like `Rgba8UnormSrgb` for RGBA.
            (ColorModel::RGB, ChannelDatatype::U8) => (
                pad_rgb_to_rgba(&image.blob, u8::MAX).into(),
                TextureFormat::Rgba8Unorm,
            ),

            (ColorModel::RGBA, ChannelDatatype::U8) => {
                (cast_slice_to_cow(&image.blob), TextureFormat::Rgba8Unorm)
            }

            _ => {
                // Fallback to general case:
                return general_texture_creation_desc_from_image(
                    debug_name,
                    image,
                    color_model,
                    datatype,
                );
            }
        };

        Texture2DCreationDesc {
            label: debug_name.into(),
            data,
            format,
            width: image.width(),
            height: image.height(),
        }
    }
}

fn depth_image_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    texture_key: u64,
    image: &ImageInfo,
    tensor_stats: &TensorStats,
) -> anyhow::Result<ColormappedTexture> {
    re_tracing::profile_function!();

    if let Some(pixel_format) = image.format.pixel_format {
        anyhow::bail!("Depth image does not support the PixelFormat {pixel_format}");
    }

    if image.format.color_model() != ColorModel::L {
        anyhow::bail!(
            "Depth image does not support the ColorModel {}",
            image.format.color_model()
        );
    }

    let datatype = image.format.datatype();

    let range = data_range(tensor_stats, datatype);

    let texture = get_or_create_texture(render_ctx, texture_key, || {
        general_texture_creation_desc_from_image(debug_name, image, ColorModel::L, datatype)
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
    image: &ImageInfo,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
) -> anyhow::Result<ColormappedTexture> {
    re_tracing::profile_function!();

    if let Some(pixel_format) = image.format.pixel_format {
        anyhow::bail!("Segmentation image does not support the PixelFormat {pixel_format}");
    }

    if image.format.color_model() != ColorModel::L {
        anyhow::bail!(
            "Segmentation image does not support the ColorModel {}",
            image.format.color_model()
        );
    }

    let datatype = image.format.datatype();

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
        general_texture_creation_desc_from_image(debug_name, image, ColorModel::L, datatype)
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

fn data_range(tensor_stats: &TensorStats, datatype: ChannelDatatype) -> [f32; 2] {
    let default_min = 0.0;
    let default_max = if datatype.is_float() {
        1.0
    } else {
        datatype.max_value()
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
    image: &'a ImageInfo,
    color_model: ColorModel,
    datatype: ChannelDatatype,
) -> Texture2DCreationDesc<'a> {
    re_tracing::profile_function!();

    let width = image.width();
    let height = image.height();

    let buf: &[u8] = image.blob.as_ref();

    let (data, format) = match color_model {
        ColorModel::L => {
            match datatype {
                ChannelDatatype::U8 => (Cow::Borrowed(buf), TextureFormat::R8Uint),
                ChannelDatatype::U16 => (Cow::Borrowed(buf), TextureFormat::R16Uint),
                ChannelDatatype::U32 => (Cow::Borrowed(buf), TextureFormat::R32Uint),
                ChannelDatatype::U64 => (
                    // wgpu doesn't support u64 textures
                    narrow_u64_to_f32s(&image.to_slice()),
                    TextureFormat::R32Float,
                ),

                ChannelDatatype::I8 => (Cow::Borrowed(buf), TextureFormat::R8Sint),
                ChannelDatatype::I16 => (Cow::Borrowed(buf), TextureFormat::R16Sint),
                ChannelDatatype::I32 => (Cow::Borrowed(buf), TextureFormat::R32Sint),
                ChannelDatatype::I64 => (
                    // wgpu doesn't support i64 textures
                    narrow_i64_to_f32s(&image.to_slice()),
                    TextureFormat::R32Float,
                ),

                ChannelDatatype::F16 => (Cow::Borrowed(buf), TextureFormat::R16Float),
                ChannelDatatype::F32 => (Cow::Borrowed(buf), TextureFormat::R32Float),
                ChannelDatatype::F64 => (
                    // wgpu doesn't support f64 textures
                    narrow_f64_to_f32s(&image.to_slice()),
                    TextureFormat::R32Float,
                ),
            }
        }

        ColorModel::RGB => {
            // There are no 3-channel textures in wgpu, so we need to pad to 4 channels.
            // What should we pad with? It depends on whether or not the shader interprets these as alpha.
            // To be safe, we pad with the MAX value of integers, and with 1.0 for floats.
            // TODO(emilk): tell the shader to ignore the alpha channel instead!

            match datatype {
                ChannelDatatype::U8 => (
                    pad_rgb_to_rgba(buf, u8::MAX).into(),
                    TextureFormat::Rgba8Uint,
                ),
                ChannelDatatype::U16 => (pad_cast_img(image, u16::MAX), TextureFormat::Rgba16Uint),
                ChannelDatatype::U32 => (pad_cast_img(image, u32::MAX), TextureFormat::Rgba32Uint),
                ChannelDatatype::U64 => (
                    pad_and_narrow_and_cast(&image.to_slice(), 1.0, |x: u64| x as f32),
                    TextureFormat::Rgba32Float,
                ),

                ChannelDatatype::I8 => (pad_cast_img(image, i8::MAX), TextureFormat::Rgba8Sint),
                ChannelDatatype::I16 => (pad_cast_img(image, i16::MAX), TextureFormat::Rgba16Sint),
                ChannelDatatype::I32 => (pad_cast_img(image, i32::MAX), TextureFormat::Rgba32Sint),
                ChannelDatatype::I64 => (
                    pad_and_narrow_and_cast(&image.to_slice(), 1.0, |x: i64| x as f32),
                    TextureFormat::Rgba32Float,
                ),

                ChannelDatatype::F16 => (
                    pad_cast_img(image, half::f16::from_f32(1.0)),
                    TextureFormat::Rgba16Float,
                ),
                ChannelDatatype::F32 => (pad_cast_img(image, 1.0_f32), TextureFormat::Rgba32Float),
                ChannelDatatype::F64 => (
                    pad_and_narrow_and_cast(&image.to_slice(), 1.0, |x: f64| x as f32),
                    TextureFormat::Rgba32Float,
                ),
            }
        }

        ColorModel::RGBA => {
            // TODO(emilk): premultiply alpha, or tell the shader to assume unmultiplied alpha

            match datatype {
                ChannelDatatype::U8 => (Cow::Borrowed(buf), TextureFormat::Rgba8Uint),
                ChannelDatatype::U16 => (Cow::Borrowed(buf), TextureFormat::Rgba16Uint),
                ChannelDatatype::U32 => (Cow::Borrowed(buf), TextureFormat::Rgba32Uint),
                ChannelDatatype::U64 => (
                    // wgpu doesn't support u64 textures
                    narrow_u64_to_f32s(&image.to_slice()),
                    TextureFormat::Rgba32Float,
                ),

                ChannelDatatype::I8 => (Cow::Borrowed(buf), TextureFormat::Rgba8Sint),
                ChannelDatatype::I16 => (Cow::Borrowed(buf), TextureFormat::Rgba16Sint),
                ChannelDatatype::I32 => (Cow::Borrowed(buf), TextureFormat::Rgba32Sint),
                ChannelDatatype::I64 => (
                    // wgpu doesn't support i64 textures
                    narrow_i64_to_f32s(&image.to_slice()),
                    TextureFormat::Rgba32Float,
                ),

                ChannelDatatype::F16 => (Cow::Borrowed(buf), TextureFormat::Rgba16Float),
                ChannelDatatype::F32 => (Cow::Borrowed(buf), TextureFormat::Rgba32Float),
                ChannelDatatype::F64 => (
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

/// Pad an RGB image to RGBA and cast the results to bytes.
fn pad_and_cast<T: Copy + bytemuck::Pod>(data: &[T], pad: T) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();
    // TODO(emilk): optimize by combining the two steps into one; avoiding one allocation and memcpy
    let padded: Vec<T> = pad_rgb_to_rgba(data, pad);
    let bytes: Vec<u8> = bytemuck::pod_collect_to_vec(&padded);
    bytes.into()
}

/// Pad an RGB image to RGBA and cast the results to bytes.
fn pad_cast_img<T: Copy + bytemuck::Pod>(img: &ImageInfo, pad: T) -> Cow<'static, [u8]> {
    pad_and_cast(&img.to_slice(), pad)
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
