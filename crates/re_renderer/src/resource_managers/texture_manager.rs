use std::num::NonZeroU32;

use crate::{
    resource_pools::{
        texture_pool::{GpuTextureHandleStrong, TextureDesc},
        WgpuResourcePools,
    },
    DebugLabel,
};

use super::{
    resource_manager::ResourceManager, ResourceHandle, ResourceLifeTime, ResourceManagerError,
};

slotmap::new_key_type! { pub struct Texture2DHandleInner; }

pub type Texture2DHandle = ResourceHandle<Texture2DHandleInner>;

pub struct Texture2D {
    label: DebugLabel,
    data: Box<[u8]>,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    //generate_mip_maps: bool, // TODO(andreas): generate mipmaps!
}

/// Texture manager for 2D textures as typically used by meshes.
///
/// The scope is intentionally limited to particular kinds of textures that currently
/// require this kind of handle abstraction/management.
/// More complex textures types are typically handled within renderer which utilize the texture pool directly.
/// This manager in contrast, deals with user provided texture data!
/// We might revisit this later and make this texture manager more general purpose.
pub struct TextureManager2D {
    manager: ResourceManager<Texture2DHandleInner, Texture2D, GpuTextureHandleStrong>,
    placeholder_texture: Texture2DHandle,
}

impl Default for TextureManager2D {
    fn default() -> Self {
        let mut manager = ResourceManager::default();
        let placeholder_texture = manager.store_resource(
            Texture2D {
                label: "placeholder".into(),
                data: Box::new([255, 255, 255, 255]),
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                width: 1,
                height: 1,
            },
            ResourceLifeTime::LongLived,
        );
        Self {
            manager,
            placeholder_texture,
        }
    }
}

impl TextureManager2D {
    /// Takes ownership of a new mesh.
    pub fn store_resource(
        &mut self,
        resource: Texture2D,
        lifetime: ResourceLifeTime,
    ) -> Texture2DHandle {
        self.manager.store_resource(resource, lifetime)
    }

    /// Returns a single pixel white pixel.
    pub fn placeholder_texture(&self) -> Texture2DHandle {
        self.placeholder_texture
    }

    /// Retrieve gpu representation of a mesh.
    ///
    /// Uploads to gpu if not already done.
    pub(crate) fn get_or_create_gpu_resource(
        &mut self,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        handle: Texture2DHandle,
    ) -> Result<GpuTextureHandleStrong, ResourceManagerError> {
        self.manager
            .get_or_create_gpu_resource(handle, |resource, _lifetime| {
                let size = wgpu::Extent3d {
                    width: resource.width,
                    height: resource.height,
                    depth_or_array_layers: 1,
                };
                let texture_handle = pools.textures.alloc(
                    device,
                    &TextureDesc {
                        label: resource.label.clone(),
                        size,
                        mip_level_count: 1, // TODO(andreas)
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: resource.format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    },
                );

                let texture = pools
                    .textures
                    .get_resource(&texture_handle)
                    .map_err(|e| ResourceManagerError::ResourcePoolError(e))?;

                let format_info = resource.format.describe();
                let width_blocks = resource.width / format_info.block_dimensions.0 as u32;
                let bytes_per_row = width_blocks * format_info.block_size as u32;

                // TODO(andreas): temp allocator for staging data?
                // We don't do any further validation of the buffer here as wgpu does so extensively.
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &texture.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &resource.data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(
                            NonZeroU32::new(bytes_per_row).expect("invalid bytes per row"),
                        ),
                        rows_per_image: None,
                    },
                    size,
                );

                Ok(texture_handle)
                // TODO(andreas): mipmap generation
            })
    }

    pub(crate) fn frame_maintenance(&mut self, frame_index: u64) {
        self.manager.frame_maintenance(frame_index);
    }
}
