// Low level resource pools for wgpu resources

pub(crate) mod bind_group_layout_pool;
pub(crate) mod bind_group_pool;
pub(crate) mod buffer_pool;
pub(crate) mod pipeline_layout_pool;
pub(crate) mod render_pipeline_pool;
pub(crate) mod sampler_pool;
pub(crate) mod shader_module_pool;
pub(crate) mod texture_pool;

mod dynamic_resource_pool;
mod resource;
mod static_resource_pool;

use self::{
    bind_group_layout_pool::GpuBindGroupLayoutPool, bind_group_pool::GpuBindGroupPool,
    buffer_pool::GpuBufferPool, pipeline_layout_pool::GpuPipelineLayoutPool,
    render_pipeline_pool::GpuRenderPipelinePool, sampler_pool::GpuSamplerPool,
    shader_module_pool::GpuShaderModulePool, texture_pool::TexturePool,
};

/// Collection of all wgpu resource pools.
///
/// Note that all resource pools define their resources by type & type properties (the descriptor).
/// This means they are not directly concerned with contents and tend to act more like allocators.
/// Garbage collection / resource reclamation strategy differs by type,
/// for details check their respective allocation/creation functions!
#[derive(Default)]
pub struct WgpuResourcePools {
    pub(crate) bind_group_layouts: GpuBindGroupLayoutPool,
    pub(crate) pipeline_layouts: GpuPipelineLayoutPool,
    pub(crate) render_pipelines: GpuRenderPipelinePool,
    pub(crate) samplers: GpuSamplerPool,
    pub(crate) shader_modules: GpuShaderModulePool,

    pub(crate) bind_groups: GpuBindGroupPool,

    pub(crate) buffers: GpuBufferPool,
    pub(crate) textures: TexturePool,
}
