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
    pub label: DebugLabel,
    /// Data padded according to wgpu rules and ready for upload.
    /// Does not contain any mipmapping.
    pub data: Vec<u8>,
    pub format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    //generate_mip_maps: bool, // TODO(andreas): generate mipmaps!
}

impl Texture2D {
    pub fn convert_rgb8_to_rgba8(rgb_pixels: &[u8]) -> Vec<u8> {
        rgb_pixels
            .chunks_exact(3)
            .flat_map(|color| [color[0], color[1], color[2], 255])
            .collect()
    }

    fn bytes_per_row(&self) -> u32 {
        let format_info = self.format.describe();
        let width_blocks = self.width / format_info.block_dimensions.0 as u32;
        width_blocks * format_info.block_size as u32
    }

    fn needs_row_alignment(&self) -> bool {
        self.data.len() as u32 > wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
            && self.data.len() as u32 % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT != 0
    }
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
    white_texture: Texture2DHandle,
}

impl Default for TextureManager2D {
    fn default() -> Self {
        let mut manager = ResourceManager::default();
        let white_texture = manager.store_resource(
            Texture2D {
                label: "placeholder".into(),
                data: vec![255, 255, 255, 255],
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                width: 1,
                height: 1,
            },
            ResourceLifeTime::LongLived,
        );
        Self {
            manager,
            white_texture,
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
        if !resource.width.is_power_of_two() || !resource.width.is_power_of_two() {
            re_log::warn!(
                "Texture {:?} has the non-power-of-two (NPOT) resolution of {}x{}.
 NPOT textures are slower and on WebGL can't handle  mipmapping, UV wrapping and UV tiling",
                resource.label,
                resource.width,
                resource.height
            );
        }
        // TODO(andreas): Should it be possible to do this from the outside? Probably have some "pre-aligned" flag on the texture.
        if resource.needs_row_alignment() {
            re_log::warn!("Texture {:?} byte rows are not aligned to {}. Will do manual alignment before gpu upload.",
                    resource.label, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        }

        self.manager.store_resource(resource, lifetime)
    }

    /// Returns a single pixel white pixel.
    pub fn white_texture(&self) -> Texture2DHandle {
        self.white_texture
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
                    .map_err(ResourceManagerError::ResourcePoolError)?;

                // Pad rows if necessary.
                let mut bytes_per_row = resource.bytes_per_row();
                let padded_rows: Vec<u8>;
                let upload_data = if resource.needs_row_alignment() {
                    let num_padding_bytes = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
                        - (bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

                    padded_rows = resource
                        .data
                        .chunks_exact(bytes_per_row as usize)
                        .flat_map(|unpadded_row| {
                            unpadded_row
                                .iter()
                                .cloned()
                                .chain(std::iter::repeat(255).take(num_padding_bytes as usize))
                        })
                        .collect();

                    bytes_per_row += num_padding_bytes;
                    &padded_rows
                } else {
                    &resource.data
                };

                // TODO(andreas): temp allocator for staging data?
                // We don't do any further validation of the buffer here as wgpu does so extensively.
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: &texture.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    upload_data,
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
