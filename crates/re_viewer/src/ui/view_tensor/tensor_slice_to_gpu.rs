use re_log_types::{component_types::TensorCastError, DecodedTensor, TensorDataType};
use re_renderer::{
    renderer::ColormappedTexture,
    resource_managers::{GpuTexture2D, Texture2DCreationDesc, TextureManager2DError},
};
use re_viewer_context::{
    gpu_bridge::{self, range, RangeError},
    TensorStats,
};

use super::{
    ui::{selected_tensor_slice, SliceSelection},
    ViewTensorState,
};

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
    render_ctx: &mut re_renderer::RenderContext,
    tensor: &DecodedTensor,
    tensor_stats: &TensorStats,
    state: &ViewTensorState,
) -> Result<ColormappedTexture, TextureManager2DError<TensorUploadError>> {
    crate::profile_function!();

    let range =
        range(tensor_stats).map_err(|err| TextureManager2DError::DataCreation(err.into()))?;
    let texture = upload_texture_slice_to_gpu(render_ctx, tensor, state.slice())?;

    let color_mapping = state.color_mapping();

    Ok(ColormappedTexture {
        texture,
        decode_srgb: false,
        range,
        gamma: color_mapping.gamma,
        color_mapper: Some(re_renderer::renderer::ColorMapper::Function(
            color_mapping.map,
        )),
    })
}

fn upload_texture_slice_to_gpu(
    render_ctx: &mut re_renderer::RenderContext,
    tensor: &DecodedTensor,
    slice_selection: &SliceSelection,
) -> Result<GpuTexture2D, TextureManager2DError<TensorUploadError>> {
    let id = egui::util::hash((tensor.id(), slice_selection));

    gpu_bridge::try_get_or_create_texture(render_ctx, id, || {
        texture_desc_from_tensor(tensor, slice_selection)
    })
}

fn texture_desc_from_tensor(
    tensor: &DecodedTensor,
    slice_selection: &SliceSelection,
) -> Result<Texture2DCreationDesc<'static>, TensorUploadError> {
    use wgpu::TextureFormat;
    crate::profile_function!();

    let tensor = tensor.inner();

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
    slice_selection: &SliceSelection,
    format: wgpu::TextureFormat,
    caster: impl Fn(From) -> To,
) -> Result<Texture2DCreationDesc<'static>, TensorUploadError> {
    crate::profile_function!();

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
        crate::profile_scope!("copy_from_slice");
        ndarray::Zip::from(pixels_view)
            .and(slice)
            .for_each(|pixel: &mut To, value: &From| {
                *pixel = caster(*value);
            });
    }

    crate::profile_scope!("pod_collect_to_vec");
    Ok(Texture2DCreationDesc {
        label: "tensor_slice".into(),
        data: bytemuck::pod_collect_to_vec(&pixels).into(),
        format,
        width: width as u32,
        height: height as u32,
    })
}
