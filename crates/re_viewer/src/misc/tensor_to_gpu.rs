use std::borrow::Cow;

use bytemuck::{allocation::pod_collect_to_vec, cast_slice, Pod};
use wgpu::TextureFormat;

use re_log_types::component_types::{Tensor, TensorData, TensorTrait as _};
use re_renderer::{
    resource_managers::{GpuTexture2DHandle, Texture2DCreationDesc},
    RenderContext,
};

// ----------------------------------------------------------------------------

pub struct ColormappedTextured {
    pub texture: GpuTexture2DHandle,
}

pub fn textured_rect_from_tensor(
    render_ctx: &mut RenderContext,
    debug_name: &str,
    tensor: &Tensor,
    annotations: &crate::ui::Annotations,
) -> anyhow::Result<ColormappedTextured> {
    crate::profile_function!();

    use re_log_types::component_types::TensorDataMeaning;

    match tensor.meaning {
        TensorDataMeaning::Unknown => {
            textured_rect_from_color_tensor(render_ctx, debug_name, tensor)
        }
        TensorDataMeaning::ClassId => {
            textured_rect_from_class_id_tensor(render_ctx, debug_name, tensor, annotations)
        }
        TensorDataMeaning::Depth => textured_rect_from_depth_tensor(render_ctx, debug_name, tensor),
    }
}

fn textured_rect_from_color_tensor(
    render_ctx: &mut RenderContext,
    debug_name: &str,
    tensor: &Tensor,
) -> anyhow::Result<ColormappedTextured> {
    let texture = get_or_create_texture(render_ctx, tensor, || {
        texture_creation_desc_from_color_tensor(debug_name, tensor)
    })?;
    Ok(ColormappedTextured { texture })
}

fn texture_creation_desc_from_color_tensor<'a>(
    debug_name: &str,
    tensor: &'a Tensor,
) -> anyhow::Result<Texture2DCreationDesc<'a>> {
    crate::profile_function!(format!(
        "dtype: {}, shape: {:?}",
        tensor.dtype(),
        tensor.shape()
    ));
    let [height, width, depth] = height_width_depth(tensor)?;
    let (data, format) = match (depth, &tensor.data) {
        // special handling for sRGB(A) textures:
        (3, TensorData::U8(buf)) => (
            pad_and_cast(buf.as_slice(), 255),
            TextureFormat::Rgba8UnormSrgb,
        ),

        (4, TensorData::U8(buf)) => (
            cast_slice_to_cow(buf.as_slice()),
            TextureFormat::Rgba8UnormSrgb,
        ),

        _ => {
            // Fallback to general case:
            return general_texture_creation_desc_from_tensor(debug_name, tensor);
        }
    };

    let desc = Texture2DCreationDesc {
        label: debug_name.into(),
        data,
        format,
        width,
        height,
    };
    Ok(desc)
}

fn textured_rect_from_class_id_tensor(
    _render_ctx: &mut RenderContext,
    _debug_name: &str,
    _tensor: &Tensor,
    _annotations: &crate::ui::Annotations,
) -> anyhow::Result<ColormappedTextured> {
    anyhow::bail!("annotations mapping not implemented")
}

fn textured_rect_from_depth_tensor(
    _render_ctx: &mut RenderContext,
    _debug_name: &str,
    _tensor: &Tensor,
) -> anyhow::Result<ColormappedTextured> {
    anyhow::bail!("depth tensors not implemented")
}

// ----------------------------------------------------------------------------

fn get_or_create_texture<'a, Err>(
    render_ctx: &mut RenderContext,
    tensor: &Tensor,
    try_create_texture_desc: impl FnOnce() -> Result<Texture2DCreationDesc<'a>, Err>,
) -> Result<GpuTexture2DHandle, Err> {
    let texture_key = tensor.id().0.as_u128() as u64;
    render_ctx.texture_manager_2d.get_or_create_with(
        texture_key,
        &mut render_ctx.gpu_resources.textures,
        try_create_texture_desc,
    )
}

fn cast_slice_to_cow<From: Pod>(slice: &[From]) -> Cow<'_, [u8]> {
    cast_slice(slice).into()
}

// wgpu doesn't support f64 textures, so we need to convert to f32:
fn cast_f64_to_f32s(slice: &[f64]) -> Cow<'static, [u8]> {
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

fn general_texture_creation_desc_from_tensor<'a>(
    debug_name: &str,
    tensor: &'a Tensor,
) -> anyhow::Result<Texture2DCreationDesc<'a>> {
    crate::profile_function!(format!(
        "dtype: {}, shape: {:?}",
        tensor.dtype(),
        tensor.shape()
    ));
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
        (1, TensorData::F64(buf)) => (cast_f64_to_f32s(buf), TextureFormat::R32Float),

        // NOTE: 2-channel images are not supported by all of Rerun yet, but are included here for completeness:
        (2, TensorData::U8(buf)) => (cast_slice_to_cow(buf.as_slice()), TextureFormat::Rg8Uint),
        (2, TensorData::I8(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg8Sint),
        (2, TensorData::U16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg16Uint),
        (2, TensorData::I16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg16Sint),
        (2, TensorData::U32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg32Uint),
        (2, TensorData::I32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg32Sint),
        // (2, TensorData::F16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg16Float), TODO(#854)
        (2, TensorData::F32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rg32Float),
        (2, TensorData::F64(buf)) => (cast_f64_to_f32s(buf), TextureFormat::Rg32Float),

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

        (4, TensorData::U8(buf)) => (cast_slice_to_cow(buf.as_slice()), TextureFormat::Rgba8Uint),
        (4, TensorData::I8(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba8Sint),
        (4, TensorData::U16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba16Uint),
        (4, TensorData::I16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba16Sint),
        (4, TensorData::U32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba32Uint),
        (4, TensorData::I32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba32Sint),
        // (4, TensorData::F16(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba16Float), TODO(#854)
        (4, TensorData::F32(buf)) => (cast_slice_to_cow(buf), TextureFormat::Rgba32Float),
        (4, TensorData::F64(buf)) => (cast_f64_to_f32s(buf), TextureFormat::Rgba32Float),

        (_depth, dtype) => {
            anyhow::bail!("Don't know how to turn a tensor of shape={:?} and dtype={dtype:?} into a color image", tensor.shape)
        }
    };

    let desc = Texture2DCreationDesc {
        label: debug_name.into(),
        data,
        format,
        width,
        height,
    };
    Ok(desc)
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

// ----------------------------------------------------------------------------
