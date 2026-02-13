use std::hash::Hash;

use super::dynamic_resource_pool::{DynamicResource, DynamicResourcePool, DynamicResourcesDesc};
use super::resource::PoolError;
use crate::debug_label::DebugLabel;

slotmap::new_key_type! { pub struct GpuTextureHandle; }

/// A reference-counter baked texture.
/// Once all instances are dropped, the texture will be marked for reclamation in the following frame.
pub type GpuTexture =
    std::sync::Arc<DynamicResource<GpuTextureHandle, TextureDesc, GpuTextureInternal>>;

pub struct GpuTextureInternal {
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

impl TextureDesc {
    /// Copies the desc but changes the label.
    pub fn with_label(&self, label: DebugLabel) -> Self {
        Self {
            label,
            ..self.clone()
        }
    }

    /// Copies the desc but adds a string to the label.
    pub fn with_label_push(&self, append_this: &str) -> Self {
        let mut copy = self.clone();
        copy.label = format!("{}{append_this}", copy.label).into();
        copy
    }
}

impl DynamicResourcesDesc for TextureDesc {
    /// Number of bytes this texture is expected to take.
    ///
    /// The actual number might be both bigger (padding) and lower (gpu sided compression).
    fn resource_size_in_bytes(&self) -> u64 {
        let mut size_in_bytes = 0;
        let block_size = self
            .format
            .block_copy_size(Some(wgpu::TextureAspect::All))
            .unwrap_or_else(|| {
                self.format
                    .block_copy_size(Some(wgpu::TextureAspect::DepthOnly))
                    .unwrap_or(0)
                    + self
                        .format
                        .block_copy_size(Some(wgpu::TextureAspect::StencilOnly))
                        .unwrap_or(0)
            });
        let block_dimension = self.format.block_dimensions();
        let pixels_per_block = block_dimension.0 as u64 * block_dimension.1 as u64;

        for mip in 0..self.size.max_mips(self.dimension) {
            let mip_size = self
                .size
                .mip_level_size(mip, self.dimension)
                .physical_size(self.format);
            let num_pixels = mip_size.width * mip_size.height * mip_size.depth_or_array_layers;
            let num_blocks = num_pixels as u64 / pixels_per_block;
            size_in_bytes += num_blocks * block_size as u64;
        }

        size_in_bytes
    }

    fn allow_reuse(&self) -> bool {
        true
    }
}

#[derive(Default)]
pub struct GpuTexturePool {
    pool: DynamicResourcePool<GpuTextureHandle, TextureDesc, GpuTextureInternal>,
}

impl GpuTexturePool {
    /// Returns a reference-counted handle to a currently unused texture.
    /// Once ownership to the handle is given up, the texture may be reclaimed in future frames.
    pub fn alloc(&self, device: &wgpu::Device, desc: &TextureDesc) -> GpuTexture {
        re_tracing::profile_function!();
        self.pool.alloc(desc, |desc| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: desc.label.get(),
                size: desc.size,
                mip_level_count: desc.mip_level_count,
                sample_count: desc.sample_count,
                dimension: desc.dimension,
                format: desc.format,
                usage: desc.usage,
                view_formats: &[desc.format],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            GpuTextureInternal {
                texture,
                default_view: view,
            }
        })
    }

    /// Called by `RenderContext` every frame. Updates statistics and may free unused textures.
    pub fn begin_frame(&mut self, frame_index: u64) {
        self.pool
            .begin_frame(frame_index, |res| res.texture.destroy());
    }

    /// Method to retrieve a resource from a weak handle (used by [`super::GpuBindGroupPool`])
    pub fn get_from_handle(&self, handle: GpuTextureHandle) -> Result<GpuTexture, PoolError> {
        self.pool.get_from_handle(handle)
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }

    pub fn total_gpu_size_in_bytes(&self) -> u64 {
        self.pool.total_resource_size_in_bytes()
    }
}
