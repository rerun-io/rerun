//! Upload [`Tensor`] to [`re_renderer`].

use anyhow::Context;

use std::borrow::Cow;

use bytemuck::{allocation::pod_collect_to_vec, cast_slice, Pod};
use egui::util::hash;
use wgpu::TextureFormat;

use re_components::{DecodedTensor, Tensor, TensorData};
use re_renderer::{
    pad_rgb_to_rgba,
    renderer::{ColorMapper, ColormappedTexture},
    resource_managers::Texture2DCreationDesc,
    RenderContext,
};

use crate::{Annotations, DefaultColor, TensorStats};

use super::{get_or_create_texture, try_get_or_create_texture};

// ----------------------------------------------------------------------------

/// Set up tensor for rendering on the GPU.
///
/// This will only upload the tensor if it isn't on the GPU already.
///
/// `tensor_stats` is used for determining the range of the texture.
// TODO(#2341): allow user to specify the range in ui.
pub fn tensor_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    tensor: &DecodedTensor,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
) -> anyhow::Result<ColormappedTexture> {
    re_tracing::profile_function!(format!(
        "meaning: {:?}, dtype: {}, shape: {:?}",
        tensor.meaning,
        tensor.dtype(),
        tensor.shape()
    ));

    use re_components::TensorDataMeaning;

    match tensor.meaning {
        TensorDataMeaning::Unknown => {
            color_tensor_to_gpu(render_ctx, debug_name, tensor, tensor_stats)
        }
        TensorDataMeaning::ClassId => {
            class_id_tensor_to_gpu(render_ctx, debug_name, tensor, tensor_stats, annotations)
        }
        TensorDataMeaning::Depth => {
            depth_tensor_to_gpu(render_ctx, debug_name, tensor, tensor_stats)
        }
    }
}

// ----------------------------------------------------------------------------
// Color textures:

fn color_tensor_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    tensor: &DecodedTensor,
    tensor_stats: &TensorStats,
) -> anyhow::Result<ColormappedTexture> {
    let [height, width, depth] = height_width_depth(tensor)?;

    let texture_handle = try_get_or_create_texture(render_ctx, hash(tensor.id()), || {
        let (data, format) = match (depth, &tensor.data) {
            // Normalize sRGB(A) textures to 0-1 range, and let the GPU premultiply alpha.
            // Why? Because premul must happen _before_ sRGB decode, so we can't
            // use a "Srgb-aware" texture like `Rgba8UnormSrgb` for RGBA.
            (3, TensorData::U8(buf)) => (
                pad_rgb_to_rgba(buf.as_slice(), u8::MAX).into(),
                TextureFormat::Rgba8Unorm,
            ),
            (4, TensorData::U8(buf)) => {
                (cast_slice_to_cow(buf.as_slice()), TextureFormat::Rgba8Unorm)
            }

            _ => {
                // Fallback to general case:
                return general_texture_creation_desc_from_tensor(debug_name, tensor);
            }
        };

        Ok(Texture2DCreationDesc {
            label: debug_name.into(),
            data,
            format,
            width,
            height,
        })
    })
    .map_err(|err| anyhow::anyhow!("Failed to create texture for color tensor: {err}"))?;

    let texture_format = texture_handle.format();

    // TODO(emilk): let the user specify the color space.
    let decode_srgb = texture_format == TextureFormat::Rgba8Unorm
        || super::tensor_decode_srgb_gamma_heuristic(tensor_stats, tensor.data.dtype(), depth)?;

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
        super::tensor_data_range_heuristic(tensor_stats, tensor.data.dtype())?
    };

    let color_mapper = if re_renderer::texture_info::num_texture_components(texture_format) == 1 {
        // Single-channel images = luminance = grayscale
        Some(ColorMapper::Function(re_renderer::Colormap::Grayscale))
    } else {
        None
    };

    // TODO(wumpf): There should be a way to specify whether a texture uses pre-multiplied alpha or not.
    // Assume that the texture is not pre-multiplied if it has an alpha channel.
    let multiply_rgb_with_alpha = depth == 4;

    Ok(ColormappedTexture {
        texture: texture_handle,
        range,
        decode_srgb,
        multiply_rgb_with_alpha,
        gamma: 1.0,
        color_mapper,
    })
}

// ----------------------------------------------------------------------------
// Textures with class_id annotations:

fn class_id_tensor_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    tensor: &DecodedTensor,
    tensor_stats: &TensorStats,
    annotations: &Annotations,
) -> anyhow::Result<ColormappedTexture> {
    let [_height, _width, depth] = height_width_depth(tensor)?;
    anyhow::ensure!(
        depth == 1,
        "Cannot apply annotations to tensor of shape {:?}",
        tensor.shape
    );
    anyhow::ensure!(
        tensor.dtype().is_integer(),
        "Only integer tensors can be annotated"
    );

    let (min, max) = tensor_stats
        .range
        .ok_or_else(|| anyhow::anyhow!("compressed_tensor!?"))?;
    anyhow::ensure!(0.0 <= min, "Negative class id");

    anyhow::ensure!(max <= 65535.0, "Too many class ids"); // we only support u8 and u16 tensors

    // We pack the colormap into a 2D texture so we don't go over the max texture size.
    // We only support u8 and u16 class ids, so 256^2 is the biggest texture we need.
    let num_colors = (max + 1.0) as usize;
    let colormap_width = 256;
    let colormap_height = (num_colors + colormap_width - 1) / colormap_width;

    let colormap_texture_handle =
        get_or_create_texture(render_ctx, hash(annotations.row_id()), || {
            let data: Vec<u8> = (0..(colormap_width * colormap_height))
                .flat_map(|id| {
                    let color = annotations
                        .resolved_class_description(Some((id as u16).into()))
                        .annotation_info()
                        .color(None, DefaultColor::TransparentBlack);
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

    let main_texture_handle = try_get_or_create_texture(render_ctx, hash(tensor.id()), || {
        general_texture_creation_desc_from_tensor(debug_name, tensor)
    })
    .map_err(|err| anyhow::anyhow!("Failed to create texture for class id tensor: {err}"))?;

    Ok(ColormappedTexture {
        texture: main_texture_handle,
        range: [0.0, (colormap_width * colormap_height) as f32],
        decode_srgb: false, // Setting this to true would affect the class ids, not the color they resolve to.
        multiply_rgb_with_alpha: false, // already premultiplied!
        gamma: 1.0,
        color_mapper: Some(ColorMapper::Texture(colormap_texture_handle)),
    })
}

// ----------------------------------------------------------------------------
// Depth textures:

pub fn depth_tensor_to_gpu(
    render_ctx: &RenderContext,
    debug_name: &str,
    tensor: &DecodedTensor,
    tensor_stats: &TensorStats,
) -> anyhow::Result<ColormappedTexture> {
    let [_height, _width, depth] = height_width_depth(tensor)?;
    anyhow::ensure!(
        depth == 1,
        "Depth tensor of weird shape: {:?}",
        tensor.shape
    );
    let (min, max) = depth_tensor_range(tensor, tensor_stats)?;

    let texture = try_get_or_create_texture(render_ctx, hash(tensor.id()), || {
        general_texture_creation_desc_from_tensor(debug_name, tensor)
    })
    .map_err(|err| anyhow::anyhow!("Failed to create depth tensor texture: {err}"))?;

    Ok(ColormappedTexture {
        texture,
        range: [min as f32, max as f32],
        decode_srgb: false,
        multiply_rgb_with_alpha: false,
        gamma: 1.0,
        color_mapper: Some(ColorMapper::Function(re_renderer::Colormap::Turbo)),
    })
}

fn depth_tensor_range(
    tensor: &DecodedTensor,
    tensor_stats: &TensorStats,
) -> anyhow::Result<(f64, f64)> {
    let range = tensor_stats.range.ok_or(anyhow::anyhow!(
        "Tensor has no range!? Was this compressed?"
    ))?;
    let (mut min, mut max) = range;

    anyhow::ensure!(
        min.is_finite() && max.is_finite(),
        "Tensor has non-finite values"
    );

    min = min.min(0.0); // Depth usually start at zero.

    if min == max {
        // Uniform image. We can't remap it to a 0-1 range, so do whatever:
        min = 0.0;
        max = if tensor.dtype().is_float() {
            1.0
        } else {
            tensor.dtype().max_value()
        };
    }

    Ok((min, max))
}

// ----------------------------------------------------------------------------

/// Uploads the tensor to a texture in a format that closely resembled the input.
/// Uses no `Unorm/Snorm` formats.
fn general_texture_creation_desc_from_tensor<'a>(
    debug_name: &str,
    tensor: &'a DecodedTensor,
) -> anyhow::Result<Texture2DCreationDesc<'a>> {
    let [height, width, depth] = height_width_depth(tensor)?;

    let (data, format) = match depth {
        1 => {
            match &tensor.data {
                TensorData::U8(buf) => (cast_slice_to_cow(buf.as_slice()), TextureFormat::R8Uint),
                TensorData::U16(buf) => (cast_slice_to_cow(buf), TextureFormat::R16Uint),
                TensorData::U32(buf) => (cast_slice_to_cow(buf), TextureFormat::R32Uint),
                TensorData::U64(buf) => (narrow_u64_to_f32s(buf), TextureFormat::R32Float), // narrowing to f32!

                TensorData::I8(buf) => (cast_slice_to_cow(buf), TextureFormat::R8Sint),
                TensorData::I16(buf) => (cast_slice_to_cow(buf), TextureFormat::R16Sint),
                TensorData::I32(buf) => (cast_slice_to_cow(buf), TextureFormat::R32Sint),
                TensorData::I64(buf) => (narrow_i64_to_f32s(buf), TextureFormat::R32Float), // narrowing to f32!

                TensorData::F16(buf) => (cast_slice_to_cow(buf), TextureFormat::R16Float),
                TensorData::F32(buf) => (cast_slice_to_cow(buf), TextureFormat::R32Float),
                TensorData::F64(buf) => (narrow_f64_to_f32s(buf), TextureFormat::R32Float), // narrowing to f32!

                TensorData::JPEG(_) => {
                    unreachable!("DecodedTensor cannot contain a JPEG")
                }
            }
        }
        2 => {
            // NOTE: 2-channel images are not supported by the shader yet, but are included here for completeness:
            match &tensor.data {
                TensorData::U8(buf) => (cast_slice_to_cow(buf.as_slice()), TextureFormat::Rg8Uint),
                TensorData::U16(buf) => (cast_slice_to_cow(buf), TextureFormat::Rg16Uint),
                TensorData::U32(buf) => (cast_slice_to_cow(buf), TextureFormat::Rg32Uint),
                TensorData::U64(buf) => (narrow_u64_to_f32s(buf), TextureFormat::Rg32Float), // narrowing to f32!

                TensorData::I8(buf) => (cast_slice_to_cow(buf), TextureFormat::Rg8Sint),
                TensorData::I16(buf) => (cast_slice_to_cow(buf), TextureFormat::Rg16Sint),
                TensorData::I32(buf) => (cast_slice_to_cow(buf), TextureFormat::Rg32Sint),
                TensorData::I64(buf) => (narrow_i64_to_f32s(buf), TextureFormat::Rg32Float), // narrowing to f32!

                TensorData::F16(buf) => (cast_slice_to_cow(buf), TextureFormat::Rg16Float),
                TensorData::F32(buf) => (cast_slice_to_cow(buf), TextureFormat::Rg32Float),
                TensorData::F64(buf) => (narrow_f64_to_f32s(buf), TextureFormat::Rg32Float), // narrowing to f32!

                TensorData::JPEG(_) => {
                    unreachable!("DecodedTensor cannot contain a JPEG")
                }
            }
        }
        3 => {
            // There are no 3-channel textures in wgpu, so we need to pad to 4 channels.
            // What should we pad with? It depends on whether or not the shader interprets these as alpha.
            // To be safe, we pad with the MAX value of integers, and with 1.0 for floats.
            // TODO(emilk): tell the shader to ignore the alpha channel instead!
            match &tensor.data {
                TensorData::U8(buf) => (
                    pad_rgb_to_rgba(buf.as_slice(), u8::MAX).into(),
                    TextureFormat::Rgba8Uint,
                ),
                TensorData::U16(buf) => (pad_and_cast(buf, u16::MAX), TextureFormat::Rgba16Uint),
                TensorData::U32(buf) => (pad_and_cast(buf, u32::MAX), TextureFormat::Rgba32Uint),
                TensorData::U64(buf) => (
                    pad_and_narrow_and_cast(buf, 1.0, |x: u64| x as f32),
                    TextureFormat::Rgba32Float,
                ),

                TensorData::I8(buf) => (pad_and_cast(buf, i8::MAX), TextureFormat::Rgba8Sint),
                TensorData::I16(buf) => (pad_and_cast(buf, i16::MAX), TextureFormat::Rgba16Sint),
                TensorData::I32(buf) => (pad_and_cast(buf, i32::MAX), TextureFormat::Rgba32Sint),
                TensorData::I64(buf) => (
                    pad_and_narrow_and_cast(buf, 1.0, |x: i64| x as f32),
                    TextureFormat::Rgba32Float,
                ),

                TensorData::F16(buf) => (
                    pad_and_cast(
                        buf,
                        re_log_types::external::arrow2::types::f16::from_f32(1.0),
                    ),
                    TextureFormat::Rgba16Float,
                ),
                TensorData::F32(buf) => (pad_and_cast(buf, 1.0), TextureFormat::Rgba32Float),
                TensorData::F64(buf) => (
                    pad_and_narrow_and_cast(buf, 1.0, |x: f64| x as f32),
                    TextureFormat::Rgba32Float,
                ),

                TensorData::JPEG(_) => {
                    unreachable!("DecodedTensor cannot contain a JPEG")
                }
            }
        }
        4 => {
            // TODO(emilk): premultiply alpha, or tell the shader to assume unmultiplied alpha
            match &tensor.data {
                TensorData::U8(buf) => {
                    (cast_slice_to_cow(buf.as_slice()), TextureFormat::Rgba8Uint)
                }
                TensorData::U16(buf) => (cast_slice_to_cow(buf), TextureFormat::Rgba16Uint),
                TensorData::U32(buf) => (cast_slice_to_cow(buf), TextureFormat::Rgba32Uint),
                TensorData::U64(buf) => (narrow_u64_to_f32s(buf), TextureFormat::Rgba32Float), // narrowing to f32!

                TensorData::I8(buf) => (cast_slice_to_cow(buf), TextureFormat::Rgba8Sint),
                TensorData::I16(buf) => (cast_slice_to_cow(buf), TextureFormat::Rgba16Sint),
                TensorData::I32(buf) => (cast_slice_to_cow(buf), TextureFormat::Rgba32Sint),
                TensorData::I64(buf) => (narrow_i64_to_f32s(buf), TextureFormat::Rgba32Float), // narrowing to f32!

                TensorData::F16(buf) => (cast_slice_to_cow(buf), TextureFormat::Rgba16Float),
                TensorData::F32(buf) => (cast_slice_to_cow(buf), TextureFormat::Rgba32Float),
                TensorData::F64(buf) => (narrow_f64_to_f32s(buf), TextureFormat::Rgba32Float), // narrowing to f32!

                TensorData::JPEG(_) => {
                    unreachable!("DecodedTensor cannot contain a JPEG")
                }
            }
        }
        depth => {
            anyhow::bail!("Cannot create texture from tensor of depth {depth}");
        }
    };

    Ok(Texture2DCreationDesc {
        label: debug_name.into(),
        data,
        format,
        width,
        height,
    })
}

fn cast_slice_to_cow<From: Pod>(slice: &[From]) -> Cow<'_, [u8]> {
    cast_slice(slice).into()
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

fn pad_and_cast<T: Copy + Pod>(data: &[T], pad: T) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();
    // TODO(emilk): optimize by combining the two steps into one; avoiding one allocation and memcpy
    let padded: Vec<T> = pad_rgb_to_rgba(data, pad);
    let bytes: Vec<u8> = pod_collect_to_vec(&padded);
    bytes.into()
}

fn pad_and_narrow_and_cast<T: Copy + Pod>(
    data: &[T],
    pad: f32,
    narrow: impl Fn(T) -> f32,
) -> Cow<'static, [u8]> {
    re_tracing::profile_function!();

    let floats: Vec<f32> = data
        .chunks_exact(3)
        .flat_map(|chunk| [narrow(chunk[0]), narrow(chunk[1]), narrow(chunk[2]), pad])
        .collect();
    pod_collect_to_vec(&floats).into()
}

// ----------------------------------------------------------------------------;

fn height_width_depth(tensor: &Tensor) -> anyhow::Result<[u32; 3]> {
    use anyhow::Context as _;

    let Some([height, width, channel]) = tensor.image_height_width_channels() else {
        anyhow::bail!("Tensor is not an image");
    };

    let [height, width] = [
        u32::try_from(height).context("Image height is too large")?,
        u32::try_from(width).context("Image width is too large")?,
    ];

    Ok([height, width, channel as u32])
}
