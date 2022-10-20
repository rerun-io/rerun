// Low level resource pools for wgpu resources

pub(crate) mod bind_group_layout_pool;
pub(crate) mod bind_group_pool;
pub(crate) mod buffer_pool;
pub(crate) mod pipeline_layout_pool;
pub(crate) mod render_pipeline_pool;
pub(crate) mod sampler_pool;
pub(crate) mod shader_module_pool;
pub(crate) mod texture_pool;

mod resource_pool;

use self::{
    bind_group_layout_pool::BindGroupLayoutPool, bind_group_pool::BindGroupPool,
    buffer_pool::BufferPool, pipeline_layout_pool::PipelineLayoutPool,
    render_pipeline_pool::RenderPipelinePool, sampler_pool::SamplerPool,
    shader_module_pool::ShaderModulePool, texture_pool::TexturePool,
};

/// Collection of all wgpu resource pools.
///
/// Note that all resource pools define their resources by type & type properties (the descriptor).
/// This means they are not directly concerned with contents and tend to act more like allocators.
#[derive(Default)]
pub(crate) struct WgpuResourcePools {
    pub(crate) bind_group_layouts: BindGroupLayoutPool,
    pub(crate) bind_groups: BindGroupPool,
    pub(crate) buffers: BufferPool,
    pub(crate) pipeline_layouts: PipelineLayoutPool,
    pub(crate) render_pipelines: RenderPipelinePool,
    pub(crate) samplers: SamplerPool,
    pub(crate) shader_modules: ShaderModulePool,
    pub(crate) textures: TexturePool,
}
