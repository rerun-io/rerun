//! Wgpu resource pools are concerned with handling low level gpu resources efficiently.
//!
//! They facilitate easy creation and avoidance of unnecessary gpu allocations.
//!
//!
//! This is in contrast to the [`crate::resource_managers`] which are concerned with
//! higher level resources that arise from processing user provided data.

mod bind_group_layout_pool;
pub use bind_group_layout_pool::{
    BindGroupLayoutDesc, GpuBindGroupLayoutHandle, GpuBindGroupLayoutPool,
};

mod bind_group_pool;
pub use bind_group_pool::{
    BindGroupDesc, BindGroupEntry, GpuBindGroupHandle, GpuBindGroupHandleStrong, GpuBindGroupPool,
};

mod buffer_pool;
pub use buffer_pool::{BufferDesc, GpuBufferHandle, GpuBufferHandleStrong, GpuBufferPool};

mod pipeline_layout_pool;
pub use pipeline_layout_pool::{
    GpuPipelineLayoutHandle, GpuPipelineLayoutPool, PipelineLayoutDesc,
};

mod render_pipeline_pool;
pub use render_pipeline_pool::{
    GpuRenderPipelineHandle, GpuRenderPipelinePool, RenderPipelineDesc, VertexBufferLayout,
};

mod sampler_pool;
pub use sampler_pool::{GpuSamplerHandle, GpuSamplerPool, SamplerDesc};

mod shader_module_pool;
pub use shader_module_pool::{GpuShaderModuleHandle, GpuShaderModulePool, ShaderModuleDesc};

mod texture_pool;
pub use texture_pool::{
    GpuTexture, GpuTextureHandle, GpuTextureHandleStrong, GpuTexturePool, TextureDesc,
};

mod resource;
pub use resource::PoolError;

mod dynamic_resource_pool;
mod static_resource_pool;

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
    pub(crate) textures: GpuTexturePool,
}
