//! Bridge to `re_renderer`

mod tensor_to_gpu;
pub use tensor_to_gpu::tensor_to_gpu;

// ----------------------------------------------------------------------------

use re_renderer::{
    resource_managers::{GpuTexture2DHandle, Texture2DCreationDesc},
    RenderContext,
};

pub fn get_or_create_texture<'a, Err>(
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
