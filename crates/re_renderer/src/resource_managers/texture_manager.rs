use crate::{
    resource_pools::texture_pool::{GpuTextureHandleStrong, TextureDesc},
    DebugLabel, RenderContext,
};

use super::{
    resource_manager::ResourceManager, ResourceHandle, ResourceLifeTime, ResourceManagerError,
};

slotmap::new_key_type! { pub struct Texture2DHandleInner; }

pub type Texture2DHandle = ResourceHandle<Texture2DHandleInner>;

#[allow(dead_code)] // TODO(andreas): WIP
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
/// This here in contrast deals with user provided texture data!
/// We might revisit this later and make this texture manager more general purpose.
#[derive(Default)]
pub struct TextureManager2D {
    manager: ResourceManager<Texture2DHandleInner, Texture2D, GpuTextureHandleStrong>,
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

    /// Retrieve gpu representation of a mesh.
    ///
    /// Uploads to gpu if not already done.
    #[allow(dead_code)] // TODO(andreas): WIP
    pub(crate) fn get_or_create_gpu_resource(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        handle: Texture2DHandle,
    ) -> Result<GpuTextureHandleStrong, ResourceManagerError> {
        ctx.texture_manager_2d
            .manager
            .get_or_create_gpu_resource(handle, |resource, _lifetime| {
                ctx.resource_pools.textures.alloc(
                    device,
                    &TextureDesc {
                        label: resource.label.clone(),
                        size: wgpu::Extent3d {
                            width: resource.width,
                            height: resource.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1, // TODO(andreas)
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: resource.format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    },
                )
                // TODO(andreas): gpu resource upload code!
                // TODO(andreas): mipmap generation
            })
    }

    pub(crate) fn frame_maintenance(&mut self, frame_index: u64) {
        self.manager.frame_maintenance(frame_index);
    }
}
