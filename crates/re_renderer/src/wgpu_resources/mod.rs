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
    BindGroupDesc, BindGroupEntry, GpuBindGroup, GpuBindGroupHandle, GpuBindGroupPool,
};

mod buffer_pool;
pub use buffer_pool::{BufferDesc, GpuBuffer, GpuBufferHandle, GpuBufferPool};

mod pipeline_layout_pool;
pub use pipeline_layout_pool::{
    GpuPipelineLayoutHandle, GpuPipelineLayoutPool, PipelineLayoutDesc,
};

mod render_pipeline_pool;
pub use render_pipeline_pool::{
    GpuRenderPipelineHandle, GpuRenderPipelinePool, GpuRenderPipelinePoolAccessor,
    GpuRenderPipelinePoolMemMoveAccessor, RenderPipelineDesc, VertexBufferLayout,
};

mod sampler_pool;
pub use sampler_pool::{GpuSamplerHandle, GpuSamplerPool, SamplerDesc};

mod shader_module_pool;
pub use shader_module_pool::{GpuShaderModuleHandle, GpuShaderModulePool, ShaderModuleDesc};

mod texture_pool;
pub use texture_pool::{
    GpuTexture, GpuTextureHandle, GpuTextureInternal, GpuTexturePool, TextureDesc,
};

mod resource;
pub use resource::PoolError;

mod dynamic_resource_pool;
mod static_resource_pool;
pub use static_resource_pool::StaticResourcePoolAccessor;

/// Collection of all wgpu resource pools.
///
/// Note that all resource pools define their resources by type & type properties (the descriptor).
/// This means they are not directly concerned with contents and tend to act more like allocators.
/// Garbage collection / resource reclamation strategy differs by type,
/// for details check their respective allocation/creation functions!
#[derive(Default)]
pub struct WgpuResourcePools {
    pub bind_group_layouts: GpuBindGroupLayoutPool,
    pub pipeline_layouts: GpuPipelineLayoutPool,
    pub render_pipelines: GpuRenderPipelinePool,
    pub samplers: GpuSamplerPool,
    pub shader_modules: GpuShaderModulePool,

    pub bind_groups: GpuBindGroupPool,

    pub buffers: GpuBufferPool,
    pub textures: GpuTexturePool,
}

#[derive(Default)]
pub struct WgpuResourcePoolStatistics {
    pub num_bind_group_layouts: usize,
    pub num_pipeline_layouts: usize,
    pub num_render_pipelines: usize,
    pub num_samplers: usize,
    pub num_shader_modules: usize,
    pub num_bind_groups: usize,
    pub num_buffers: usize,
    pub num_textures: usize,
    pub total_buffer_size_in_bytes: u64,
    pub total_texture_size_in_bytes: u64,
}

impl WgpuResourcePoolStatistics {
    pub fn total_bytes(&self) -> u64 {
        let Self {
            num_bind_group_layouts: _,
            num_pipeline_layouts: _,
            num_render_pipelines: _,
            num_samplers: _,
            num_shader_modules: _,
            num_bind_groups: _,
            num_buffers: _,
            num_textures: _,
            total_buffer_size_in_bytes,
            total_texture_size_in_bytes,
        } = self;
        total_buffer_size_in_bytes + total_texture_size_in_bytes
    }
}

impl WgpuResourcePools {
    pub fn statistics(&self) -> WgpuResourcePoolStatistics {
        WgpuResourcePoolStatistics {
            num_bind_group_layouts: self.bind_group_layouts.num_resources(),
            num_pipeline_layouts: self.pipeline_layouts.num_resources(),
            num_render_pipelines: self.render_pipelines.num_resources(),
            num_samplers: self.samplers.num_resources(),
            num_shader_modules: self.shader_modules.num_resources(),
            num_bind_groups: self.bind_groups.num_resources(),
            num_buffers: self.buffers.num_resources(),
            num_textures: self.textures.num_resources(),
            total_buffer_size_in_bytes: self.buffers.total_gpu_size_in_bytes(),
            total_texture_size_in_bytes: self.textures.total_gpu_size_in_bytes(),
        }
    }
}
