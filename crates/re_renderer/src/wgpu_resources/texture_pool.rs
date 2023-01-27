use std::hash::Hash;

use crate::debug_label::DebugLabel;

use super::{
    dynamic_resource_pool::{DynamicResourcePool, SizedResourceDesc},
    resource::PoolError,
};

slotmap::new_key_type! { pub struct GpuTextureHandle; }

/// A reference counter baked texture handle.
/// Once all strong handles are dropped, the texture will be marked for reclamation in the following frame.
pub type GpuTextureHandleStrong = std::sync::Arc<GpuTextureHandle>;

pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub default_view: wgpu::TextureView,
    // TODO(andreas) What about custom views? Should probably have a separate resource manager for it!
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct TextureDesc {
    /// Debug label of the texture. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    /// Size of the texture. All components must be greater than zero. For a
    /// regular 1D/2D texture, the unused sizes will be 1. For 2DArray textures,
    /// Z is the number of 2D textures in that array.
    pub size: wgpu::Extent3d,

    /// Mip count of texture. For a texture with no extra mips, this must be 1.
    pub mip_level_count: u32,

    /// Sample count of texture. If this is not 1, texture must have [`wgpu::BindingType::Texture::multisampled`] set to true.
    pub sample_count: u32,

    /// Dimensions of the texture.
    pub dimension: wgpu::TextureDimension,

    /// Format of the texture.
    pub format: wgpu::TextureFormat,

    /// Allowed usages of the texture. If used in other ways, the operation will panic.
    pub usage: wgpu::TextureUsages,
}

impl SizedResourceDesc for TextureDesc {
    /// Number of bytes this texture is expected to take.
    ///
    /// The actual number might be both bigger (padding) and lower (gpu sided compression).
    fn resource_size_in_bytes(&self) -> u64 {
        let mut size_in_bytes = 0;
        let format_desc = self.format.describe();
        let pixels_per_block =
            format_desc.block_dimensions.0 as u64 * format_desc.block_dimensions.1 as u64;

        for mip in 0..self.size.max_mips(self.dimension) {
            let mip_size = self
                .size
                .mip_level_size(mip, self.dimension == wgpu::TextureDimension::D3)
                .physical_size(self.format);
            let num_pixels = mip_size.width * mip_size.height * mip_size.depth_or_array_layers;
            let num_blocks = num_pixels as u64 / pixels_per_block;
            size_in_bytes += num_blocks * format_desc.block_size as u64;
        }

        size_in_bytes
    }
}

impl TextureDesc {
    fn to_wgpu_desc(&self) -> wgpu::TextureDescriptor<'_> {
        wgpu::TextureDescriptor {
            label: self.label.get(),
            size: self.size,
            mip_level_count: self.mip_level_count,
            sample_count: self.sample_count,
            dimension: self.dimension,
            format: self.format,
            usage: self.usage,
        }
    }
}

#[derive(Default)]
pub struct GpuTexturePool {
    pool: DynamicResourcePool<GpuTextureHandle, TextureDesc, GpuTexture>,
}

impl GpuTexturePool {
    /// Returns a ref counted handle to a currently unused texture.
    /// Once ownership to the handle is given up, the texture may be reclaimed in future frames.
    pub fn alloc(&mut self, device: &wgpu::Device, desc: &TextureDesc) -> GpuTextureHandleStrong {
        crate::profile_function!();
        self.pool.alloc(desc, |desc| {
            let texture = device.create_texture(&desc.to_wgpu_desc());
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            GpuTexture {
                texture,
                default_view: view,
            }
        })
    }

    /// Called by `RenderContext` every frame. Updates statistics and may free unused textures.
    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool.frame_maintenance(frame_index);
    }

    /// Takes strong texture handle to ensure the user is still holding on to the texture.
    pub fn get_resource(&self, handle: &GpuTextureHandleStrong) -> Result<&GpuTexture, PoolError> {
        self.pool.get_resource(**handle)
    }

    /// Internal method to retrieve a resource with a weak handle (used by [`super::GpuBindGroupPool`]).
    pub(super) fn get_resource_weak(
        &self,
        handle: GpuTextureHandle,
    ) -> Result<&GpuTexture, PoolError> {
        self.pool.get_resource(handle)
    }

    /// Internal method to retrieve a strong handle from a weak handle (used by [`super::GpuBindGroupPool`])
    /// without inrementing the ref-count (note the returned reference!).
    pub(super) fn get_strong_handle(&self, handle: GpuTextureHandle) -> &GpuTextureHandleStrong {
        self.pool.get_strong_handle(handle)
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }

    pub fn total_gpu_size_in_bytes(&self) -> u64 {
        self.pool.total_resource_size_in_bytes()
    }
}
