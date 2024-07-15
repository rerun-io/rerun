use re_chunk_store::RowId;
use re_renderer::{
    renderer::{ColormappedTexture, ShaderDecoding},
    resource_managers::{GpuTexture2D, Texture2DCreationDesc, TextureManager2DError},
};
use re_types::{
    blueprint::archetypes::TensorSliceSelection,
    components::{Colormap, GammaCorrection},
    datatypes::{TensorBuffer, TensorData},
    tensor_data::{TensorCastError, TensorDataType},
};
use re_viewer_context::{
    gpu_bridge::{self, colormap_to_re_renderer, tensor_data_range_heuristic, RangeError},
    TensorStats,
};

use crate::space_view_class::selected_tensor_slice;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum TensorUploadError {
    #[error(transparent)]
    TensorCastError(#[from] TensorCastError),

    #[error("Expected a 2D slice")]
    Not2D,

    #[error(transparent)]
    RangeError(#[from] RangeError),
}

pub fn colormapped_texture(
    render_ctx: &re_renderer::RenderContext,
    tensor_data_row_id: RowId,
    tensor: &TensorData,
    tensor_stats: &TensorStats,
    slice_selection: &TensorSliceSelection,
    colormap: Colormap,
    gamma: GammaCorrection,
) -> Result<ColormappedTexture, TextureManager2DError<TensorUploadError>> {
    re_tracing::profile_function!();

    let range = tensor_data_range_heuristic(tensor_stats, tensor.dtype())
        .map_err(|err| TextureManager2DError::DataCreation(err.into()))?;
    let texture =
        upload_texture_slice_to_gpu(render_ctx, tensor_data_row_id, tensor, slice_selection)?;

    Ok(ColormappedTexture {
        texture,
        range,
        decode_srgb: false,
        multiply_rgb_with_alpha: false,
        gamma: *gamma.0,
        color_mapper: re_renderer::renderer::ColorMapper::Function(colormap_to_re_renderer(
            colormap,
        )),
        shader_decoding: match tensor.buffer {
            TensorBuffer::Nv12(_) => Some(ShaderDecoding::Nv12),
            TensorBuffer::Yuy2(_) => Some(ShaderDecoding::Yuy2),
            _ => None,
        },
    })
}

fn upload_texture_slice_to_gpu(
    render_ctx: &re_renderer::RenderContext,
    tensor_data_row_id: RowId,
    tensor: &TensorData,
    slice_selection: &TensorSliceSelection,
) -> Result<GpuTexture2D, TextureManager2DError<TensorUploadError>> {
    let id = egui::util::hash((tensor_data_row_id, slice_selection));

    gpu_bridge::try_get_or_create_texture(render_ctx, id, || {
        texture_desc_from_tensor(tensor, slice_selection)
    })
}

fn texture_desc_from_tensor(
    tensor: &TensorData,
    slice_selection: &TensorSliceSelection,
) -> Result<Texture2DCreationDesc<'static>, TensorUploadError> {
    use wgpu::TextureFormat;
    re_tracing::profile_function!();

    match tensor.dtype() {
        TensorDataType::U8 => {
            let tensor = ndarray::ArrayViewD::<u8>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R8Uint, |x| x)
        }
        TensorDataType::U16 => {
            let tensor = ndarray::ArrayViewD::<u16>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R16Uint, |x| x)
        }
        TensorDataType::U32 => {
            let tensor = ndarray::ArrayViewD::<u32>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R32Uint, |x| x)
        }
        TensorDataType::U64 => {
            // narrow to f32:
            let tensor = ndarray::ArrayViewD::<u64>::try_from(tensor)?;
            to_texture_desc(
                &tensor,
                slice_selection,
                TextureFormat::R32Float,
                |x: u64| x as f32,
            )
        }
        TensorDataType::I8 => {
            let tensor = ndarray::ArrayViewD::<i8>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R8Sint, |x| x)
        }
        TensorDataType::I16 => {
            let tensor = ndarray::ArrayViewD::<i16>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R16Sint, |x| x)
        }
        TensorDataType::I32 => {
            let tensor = ndarray::ArrayViewD::<i32>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R32Sint, |x| x)
        }
        TensorDataType::I64 => {
            // narrow to f32:
            let tensor = ndarray::ArrayViewD::<i64>::try_from(tensor)?;
            to_texture_desc(
                &tensor,
                slice_selection,
                TextureFormat::R32Float,
                |x: i64| x as f32,
            )
        }
        TensorDataType::F16 => {
            let tensor = ndarray::ArrayViewD::<half::f16>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R16Float, |x| x)
        }
        TensorDataType::F32 => {
            let tensor = ndarray::ArrayViewD::<f32>::try_from(tensor)?;
            to_texture_desc(&tensor, slice_selection, TextureFormat::R32Float, |x| x)
        }
        TensorDataType::F64 => {
            // narrow to f32:
            let tensor = ndarray::ArrayViewD::<f64>::try_from(tensor)?;
            to_texture_desc(
                &tensor,
                slice_selection,
                TextureFormat::R32Float,
                |x: f64| x as f32,
            )
        }
    }
}

fn to_texture_desc<From: Copy, To: bytemuck::Pod>(
    tensor: &ndarray::ArrayViewD<'_, From>,
    slice_selection: &TensorSliceSelection,
    format: wgpu::TextureFormat,
    caster: impl Fn(From) -> To,
) -> Result<Texture2DCreationDesc<'static>, TensorUploadError> {
    re_tracing::profile_function!();

    use ndarray::Dimension as _;

    let slice = selected_tensor_slice(slice_selection, tensor);
    let slice = slice
        .into_dimensionality::<ndarray::Ix2>()
        .map_err(|_err| TensorUploadError::Not2D)?;

    let (height, width) = slice.raw_dim().into_pattern();
    let mut pixels: Vec<To> = vec![To::zeroed(); height * width];
    let pixels_view = ndarray::ArrayViewMut2::from_shape(slice.raw_dim(), pixels.as_mut_slice())
        .expect("Mismatched length.");

    {
        re_tracing::profile_scope!("copy_from_slice");
        ndarray::Zip::from(pixels_view)
            .and(slice)
            .for_each(|pixel: &mut To, value: &From| {
                *pixel = caster(*value);
            });
    }

    re_tracing::profile_scope!("pod_collect_to_vec");
    Ok(Texture2DCreationDesc {
        label: "tensor_slice".into(),
        data: bytemuck::pod_collect_to_vec(&pixels).into(),
        format,
        width: width as u32,
        height: height as u32,
    })
}
