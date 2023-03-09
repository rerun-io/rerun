use re_log_types::component_types::{Tensor, TensorData, TensorTrait};
use re_renderer::{
    resource_managers::{GpuTexture2DHandle, Texture2DCreationDesc},
    RenderContext,
};

pub fn texture_from_tensor(
    render_ctx: &mut RenderContext,
    tensor: &Tensor,
) -> Option<GpuTexture2DHandle> {
    let label = format!("tensor {:?}", tensor.shape()).into();

    if !tensor.is_shaped_like_an_image() {
        return None;
    }

    let shape = &tensor.shape();

    let [height, width] = [u32::try_from(shape[0].size), u32::try_from(shape[1].size)];

    let (Ok(height), Ok(width)) = (height, width) else {
        return None;
    };

    let format = match tensor.dtype() {
        re_log_types::TensorDataType::U8 => todo!(),
        re_log_types::TensorDataType::U16 => todo!(),
        re_log_types::TensorDataType::U32 => todo!(),
        re_log_types::TensorDataType::U64 => todo!(),
        re_log_types::TensorDataType::I8 => todo!(),
        re_log_types::TensorDataType::I16 => todo!(),
        re_log_types::TensorDataType::I32 => todo!(),
        re_log_types::TensorDataType::I64 => todo!(),
        re_log_types::TensorDataType::F16 => todo!(),
        re_log_types::TensorDataType::F32 => todo!(),
        re_log_types::TensorDataType::F64 => todo!(),
    };

    let data = match &tensor.data {
        TensorData::JPEG(_) => return None,
        TensorData::U8(buf) => bytemuck::cast_slice(buf.as_slice()),
        TensorData::U16(buf) => bytemuck::cast_slice(buf.as_slice()),
        TensorData::U32(buf) => bytemuck::cast_slice(buf.as_slice()),
        TensorData::U64(buf) => bytemuck::cast_slice(buf.as_slice()),
        TensorData::I8(buf) => bytemuck::cast_slice(buf.as_slice()),
        TensorData::I16(buf) => bytemuck::cast_slice(buf.as_slice()),
        TensorData::I32(buf) => bytemuck::cast_slice(buf.as_slice()),
        TensorData::I64(buf) => bytemuck::cast_slice(buf.as_slice()),
        TensorData::F32(buf) => bytemuck::cast_slice(buf.as_slice()),
        TensorData::F64(buf) => bytemuck::cast_slice(buf.as_slice()),
    };

    let renderer_texture_handle = render_ctx.texture_manager_2d.create(
        &mut render_ctx.gpu_resources.textures,
        &Texture2DCreationDesc {
            label,
            data,
            format,
            width,
            height,
        },
    );

    Some(renderer_texture_handle)
}
