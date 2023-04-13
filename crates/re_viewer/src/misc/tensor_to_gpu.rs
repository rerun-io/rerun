use std::borrow::Cow;

use bytemuck::{allocation::pod_collect_to_vec, cast_slice, Pod};
use egui::util::hash;
use wgpu::TextureFormat;

use re_log_types::component_types::{Tensor, TensorData};
use re_renderer::{
    renderer::{ColorMapper, ColormappedTexture},
    resource_managers::{GpuTexture2DHandle, Texture2DCreationDesc},
    RenderContext,
};

use super::caches::TensorStats;

// ----------------------------------------------------------------------------

/// Set up tensor for rendering on the GPU.
///
/// This will only upload the tensor if it isn't on the GPU already.
///
/// `tensor_stats` is used for determining the range of the texture.
// TODO(emilk): allow user to specify the range in ui.
pub fn tensor_to_gpu(
    render_ctx: &mut RenderContext,
    debug_name: &str,
    tensor: &Tensor,
    tensor_stats: &TensorStats,
    annotations: &crate::ui::Annotations,
) -> anyhow::Result<ColormappedTexture> {
    crate::profile_function!(format!(
        "meaning: {:?}, dtype: {}, shape: {:?}",
        tensor.meaning,
        tensor.dtype(),
        tensor.shape()
    ));

    use re_log_types::component_types::TensorDataMeaning;

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
    render_ctx: &mut RenderContext,
    debug_name: &str,
    tensor: &Tensor,
    tensor_stats: &TensorStats,
) -> anyhow::Result<ColormappedTexture> {
    let texture_handle = get_or_create_texture(render_ctx, hash(tensor.id()), || {
        let [height, width, depth] = height_width_depth(tensor)?;
        let (data, format) = match (depth, &tensor.data) {
            // Use R8Unorm and R8Snorm to get filtering on the GPU:
            (1, TensorData::U8(buf)) => (cast_slice_to_cow(buf.as_slice()), TextureFormat::R8Unorm),
            (1, TensorData::I8(buf)) => (cast_slice_to_cow(buf), TextureFormat::R8Snorm),

            // Special handling for sRGB(A) textures:
            (3, TensorData::U8(buf)) => (
                pad_and_cast(buf.as_slice(), 255),
                TextureFormat::Rgba8UnormSrgb,
            ),
            (4, TensorData::U8(buf)) => (
                // TODO(emilk): premultiply alpha
                cast_slice_to_cow(buf.as_slice()),
                TextureFormat::Rgba8UnormSrgb,
            ),

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
    })?;

    let gpu_texture = render_ctx.texture_manager_2d.get(&texture_handle)?;
    let texture_format = gpu_texture.creation_desc.format;

    // Special casing for normalized textures used above:
    let range = if matches!(
        texture_format,
        TextureFormat::R8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        [0.0, 1.0]
    } else if texture_format == TextureFormat::R8Snorm {
        [-1.0, 1.0]
    } else {
        // For instance: 16-bit images.
        // TODO(emilk): consider assuming [0-1] range for all float tensors.
        let (min, max) = tensor_stats
            .range
            .ok_or_else(|| anyhow::anyhow!("missing tensor range. compressed?"))?;
        [min as f32, max as f32]
    };

    let color_mapper = if texture_format.describe().components == 1 {
        // Single-channel images = luminance = grayscale
        Some(ColorMapper::Function(re_renderer::Colormap::Grayscale))
    } else {
        None
    };

    Ok(ColormappedTexture {
        texture: texture_handle,
        range,
        color_mapper,
    })
}

// ----------------------------------------------------------------------------
// Textures with class_id annotations:

fn class_id_tensor_to_gpu(
    render_ctx: &mut RenderContext,
    debug_name: &str,
    tensor: &Tensor,
    tensor_stats: &TensorStats,
    annotations: &crate::ui::Annotations,
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

    // create a lookup texture for the colors that's 256 wide,
    // and as many rows as needed to fit all the classes.
    anyhow::ensure!(max <= 65535.0, "Too many class ids");

    let colormap_width = 256;
    let colormap_height = (max as usize + colormap_width - 1) / colormap_width;

    let colormap_texture_handle = get_or_create_texture(
        render_ctx,
        hash(annotations.row_id),
        || -> anyhow::Result<_> {
            let data: Vec<u8> = (0..(colormap_width * colormap_height))
                .flat_map(|id| {
                    let color = annotations
                        .class_description(Some(re_log_types::component_types::ClassId(id as u16)))
                        .annotation_info()
                        .color(None, crate::ui::DefaultColor::TransparentBlack);
                    color.to_array() // premultiplied!
                })
                .collect();

            Ok(Texture2DCreationDesc {
                label: "class_id_colormap".into(),
                data: data.into(),
                format: TextureFormat::Rgba8UnormSrgb,
                width: colormap_width as u32,
                height: colormap_height as u32,
            })
        },
    )?;

    let main_texture_handle = get_or_create_texture(render_ctx, hash(tensor.id()), || {
        general_texture_creation_desc_from_tensor(debug_name, tensor)
    })?;

    Ok(ColormappedTexture {
        texture: main_texture_handle,
        range: [0.0, (colormap_width * colormap_height) as f32],
        color_mapper: Some(ColorMapper::Texture(colormap_texture_handle)),
    })
}

// ----------------------------------------------------------------------------
// Depth textures:

fn depth_tensor_to_gpu(
    render_ctx: &mut RenderContext,
    debug_name: &str,
    tensor: &Tensor,
    tensor_stats: &TensorStats,
) -> anyhow::Result<ColormappedTexture> {
    let [_height, _width, depth] = height_width_depth(tensor)?;
    anyhow::ensure!(
        depth == 1,
        "Depth tensor of weird shape: {:?}",
        tensor.shape
    );
    let (min, max) = depth_tensor_range(tensor, tensor_stats)?;

    let texture = get_or_create_texture(render_ctx, hash(tensor.id()), || {
        general_texture_creation_desc_from_tensor(debug_name, tensor)
    })?;

    Ok(ColormappedTexture {
        texture,
        range: [min as f32, max as f32],
        // TODO(emilk): make this configurable in the UI
        color_mapper: Some(ColorMapper::Function(re_renderer::Colormap::Turbo)),
    })
}

fn depth_tensor_range(tensor: &Tensor, tensor_stats: &TensorStats) -> anyhow::Result<(f64, f64)> {
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
    tensor: &'a Tensor,
) -> anyhow::Result<Texture2DCreationDesc<'a>> {
    let [height, width, depth] = height_width_depth(tensor)?;
    let (data, format) = match (depth, &tensor.data) {
        (1, TensorData::U8(buf)) => (cast_slice_to_cow(buf.as_slice()), TextureFormat::R8Uint),
        (1, TensorData::I8(buf)) => (cast_slice_to_cow(buf), TextureFormat::R8Sint),
        (1, TensorData::U16(buf)) => (cast_slice_to_cow(buf), TextureFormat::R16Uint),
        (1, TensorData::I16(buf)) => (cast_slice_to_cow(buf), TextureFormat::R16Sint),
        (1, TensorData::U32(buf)) => (cast_slice_to_cow(buf), TextureFormat::R32Uint),
        (1, TensorData::I32(buf)) => (cast_slice_to_cow(buf), TextureFormat::R32Sint),
        // (1, TensorData::F16(buf)) => (cast_slice_to_cow(buf), TextureFormat::R16Float), TODO(#854)
        (1, TensorData::F32(buf)) => (cast_slice_to_cow(buf), TextureFormat::R32Float),
        (1, TensorData::F64(buf)) => (narrow_f64_to_f32s(buf), TextureFormat::R32Float),

        // NOTE: 2-channel images are not supported by the shader yet, but are included here for completeness:
        (2, TensorData::U8(buf)) => (cast_slice_to_cow(buf.as_slice()), TextureFormat::Rg8Uint),
        (2, TensorData::I8(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg8Sint),
        (2, TensorData::U16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg16Uint),
        (2, TensorData::I16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg16Sint),
        (2, TensorData::U32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg32Uint),
        (2, TensorData::I32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg32Sint),
        // (2, TensorData::F16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg16Float), TODO(#854)
        (2, TensorData::F32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg32Float),
        (2, TensorData::F64(buf)) => (narrow_f64_to_f32s(buf), TextureFormat::Rg32Float),

        // There are no 3-channel textures in wgpu, so we need to pad to 4 channels:
        (3, TensorData::U8(buf)) => (pad_and_cast(buf.as_slice(), 0), TextureFormat::Rgba8Uint),
        (3, TensorData::I8(buf)) => (pad_and_cast(buf, 0), TextureFormat::Rgba8Sint),
        (3, TensorData::U16(buf)) => (pad_and_cast(buf, 0), TextureFormat::Rgba16Uint),
        (3, TensorData::I16(buf)) => (pad_and_cast(buf, 0), TextureFormat::Rgba16Sint),
        (3, TensorData::U32(buf)) => (pad_and_cast(buf, 0), TextureFormat::Rgba32Uint),
        (3, TensorData::I32(buf)) => (pad_and_cast(buf, 0), TextureFormat::Rgba32Sint),
        // (3, TensorData::F16(buf)) => (pad_and_cast(buf, 0.0), TextureFormat::Rgba16Float), TODO(#854)
        (3, TensorData::F32(buf)) => (pad_and_cast(buf, 0.0), TextureFormat::Rgba32Float),
        (3, TensorData::F64(buf)) => {
            let pad = 0.0;
            let floats: Vec<f32> = buf
                .chunks_exact(3)
                .flat_map(|chunk| [chunk[0] as f32, chunk[1] as f32, chunk[2] as f32, pad])
                .collect();
            (
                pod_collect_to_vec(&floats).into(),
                TextureFormat::Rgba32Float,
            )
        }

        // TODO(emilk): premultiply alpha
        (4, TensorData::U8(buf)) => (cast_slice_to_cow(buf.as_slice()), TextureFormat::Rgba8Uint),
        (4, TensorData::I8(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba8Sint),
        (4, TensorData::U16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba16Uint),
        (4, TensorData::I16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba16Sint),
        (4, TensorData::U32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba32Uint),
        (4, TensorData::I32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba32Sint),
        // (4, TensorData::F16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba16Float), TODO(#854)
        (4, TensorData::F32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba32Float),
        (4, TensorData::F64(buf)) => (narrow_f64_to_f32s(buf), TextureFormat::Rgba32Float),

        // TODO(emilk): U64/I64
        (_depth, dtype) => {
            anyhow::bail!("Don't know how to turn a tensor of shape={:?} and dtype={dtype:?} into a color image", tensor.shape)
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

fn get_or_create_texture<'a, Err>(
    render_ctx: &mut RenderContext,
    texture_key: u64,
    try_create_texture_desc: impl FnOnce() -> Result<Texture2DCreationDesc<'a>, Err>,
) -> Result<GpuTexture2DHandle, Err> {
    render_ctx.texture_manager_2d.get_or_create_with(
        texture_key,
        &mut render_ctx.gpu_resources.textures,
        try_create_texture_desc,
    )
}

fn cast_slice_to_cow<From: Pod>(slice: &[From]) -> Cow<'_, [u8]> {
    cast_slice(slice).into()
}

// wgpu doesn't support f64 textures, so we need to narrow to f32:
fn narrow_f64_to_f32s(slice: &[f64]) -> Cow<'static, [u8]> {
    crate::profile_function!();
    let f32s: Vec<f32> = slice.iter().map(|&f| f as f32).collect::<Vec<f32>>();
    let bytes: Vec<u8> = pod_collect_to_vec(&f32s);
    bytes.into()
}

fn pad_to_four_elements<T: Copy>(data: &[T], pad: T) -> Vec<T> {
    crate::profile_function!();
    data.chunks_exact(3)
        .flat_map(|chunk| [chunk[0], chunk[1], chunk[2], pad])
        .collect::<Vec<T>>()
}

fn pad_and_cast<T: Copy + Pod>(data: &[T], pad: T) -> Cow<'static, [u8]> {
    crate::profile_function!();
    let padded: Vec<T> = pad_to_four_elements(data, pad);
    let bytes: Vec<u8> = pod_collect_to_vec(&padded);
    bytes.into()
}

// ----------------------------------------------------------------------------;

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
