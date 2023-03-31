//! Wgpu resource pools are concerned with handling low level gpu resources efficiently.
//!
//! They facilitate easy creation and avoidance of unnecessary gpu allocations.
//!
//!
//! This is in contrast to the [`crate::resource_managers`] which are concerned with
//! higher level resources that arise from processing user provided data.

mod bind_group_layout_pool;
use std::borrow::Cow;

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
    GpuRenderPipelineHandle, GpuRenderPipelinePool, RenderPipelineDesc, VertexBufferLayout,
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

/// Utility for dealing with buffers containing raw 2D texture data.
#[derive(Clone)]
pub struct Texture2DBufferInfo {
    /// How many bytes per row contain actual data.
    pub bytes_per_row_unpadded: u32,

    /// How many bytes per row are required to be allocated in total.
    ///
    /// Padding bytes are always at the end of a row.
    pub bytes_per_row_padded: u32,

    /// Size required for an unpadded buffer.
    pub buffer_size_unpadded: wgpu::BufferAddress,

    /// Size required for a padded buffer as it is read/written from/to the GPU.
    pub buffer_size_padded: wgpu::BufferAddress,
}

impl Texture2DBufferInfo {
    #[inline]
    pub fn new(format: wgpu::TextureFormat, extent: glam::UVec2) -> Self {
        let format_info = format.describe();

        let width_blocks = extent.x / format_info.block_dimensions.0 as u32;
        let height_blocks = extent.y / format_info.block_dimensions.1 as u32;

        let bytes_per_row_unpadded = width_blocks * format_info.block_size as u32;
        let bytes_per_row_padded =
            wgpu::util::align_to(bytes_per_row_unpadded, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

        Self {
            bytes_per_row_unpadded,
            bytes_per_row_padded,
            buffer_size_unpadded: (bytes_per_row_unpadded * height_blocks) as wgpu::BufferAddress,
            buffer_size_padded: (bytes_per_row_padded * height_blocks) as wgpu::BufferAddress,
        }
    }

    #[inline]
    pub fn num_rows(&self) -> u32 {
        self.buffer_size_padded as u32 / self.bytes_per_row_padded
    }

    /// Removes the padding from a buffer containing gpu texture data.
    ///
    /// The buffer have the expected size for a padded buffer to hold the texture data.
    ///
    /// Note that if you're passing in gpu data, there no alignment guarantees on the returned slice,
    /// do NOT convert it using [`bytemuck`]. Use [`Texture2DBufferInfo::remove_padding_and_convert`] instead.
    pub fn remove_padding<'a>(&self, buffer: &'a [u8]) -> Cow<'a, [u8]> {
        crate::profile_function!();

        assert!(buffer.len() as wgpu::BufferAddress == self.buffer_size_padded);

        if self.bytes_per_row_padded == self.bytes_per_row_unpadded {
            return Cow::Borrowed(buffer);
        }

        let mut unpadded_buffer = Vec::with_capacity(self.buffer_size_unpadded as _);

        for row in 0..self.num_rows() {
            let offset = (self.bytes_per_row_padded * row) as usize;
            unpadded_buffer.extend_from_slice(
                &buffer[offset..(offset + self.bytes_per_row_unpadded as usize)],
            );
        }

        unpadded_buffer.into()
    }

    /// Removes the padding from a buffer containing gpu texture data and remove convert to a given type.
    ///
    /// The buffer have the expected size for a padded buffer to hold the texture data.
    ///
    /// The unpadded row size is expected to be a multiple of the size of the target type.
    /// (Which means that, while uncommon, it technically doesn't need to be as big as a block in the pixel - this can be useful for e.g. packing wide bitfields)
    pub fn remove_padding_and_convert<T: bytemuck::Pod>(&self, buffer: &[u8]) -> Vec<T> {
        crate::profile_function!();

        assert!(buffer.len() as wgpu::BufferAddress == self.buffer_size_padded);
        assert!(self.bytes_per_row_unpadded % std::mem::size_of::<T>() as u32 == 0);

        // Due to https://github.com/gfx-rs/wgpu/issues/3508 the data might be completely unaligned,
        // so much, that we can't even interpret it as e.g. a u32 slice.
        // Therefore, we have to do a copy of the data regardless of whether it's padded or not.

        let mut unpadded_buffer: Vec<T> = vec![
            T::zeroed();
            (self.num_rows() * self.bytes_per_row_unpadded / std::mem::size_of::<T>() as u32)
                as usize
        ]; // Consider using unsafe set_len() instead of vec![] to avoid zeroing the memory.
        let unpaadded_buffer_u8 = bytemuck::cast_slice_mut(&mut unpadded_buffer);

        for row in 0..self.num_rows() {
            let offset_padded = (self.bytes_per_row_padded * row) as usize;
            let offset_unpadded = (self.bytes_per_row_unpadded * row) as usize;
            unpaadded_buffer_u8
                [offset_unpadded..(offset_unpadded + self.bytes_per_row_unpadded as usize)]
                .copy_from_slice(
                    &buffer[offset_padded..(offset_padded + self.bytes_per_row_unpadded as usize)],
                );
        }

        unpadded_buffer
    }
}
