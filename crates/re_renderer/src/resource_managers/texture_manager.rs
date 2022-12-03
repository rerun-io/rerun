use std::{num::NonZeroU32, sync::Arc};

use crate::{
    wgpu_resources::{GpuTextureHandleStrong, GpuTexturePool, TextureDesc},
    DebugLabel,
};

use super::{
    resource_manager::ResourceManager, ResourceHandle, ResourceLifeTime, ResourceManagerError,
};

slotmap::new_key_type! { pub struct Texture2DHandleInner; }

pub type GpuTexture2DHandle = ResourceHandle<Texture2DHandleInner>; // TODO: Make this an alias/struct for strong texture handle.

/// Data required to create a texture 2d resource.
///
/// It is *not* stored along side the resulting texture resource!
pub struct Texture2DCreationDesc {
    pub label: DebugLabel,
    /// Data for the highest mipmap level.
    /// Must be padded according to wgpu rules and ready for upload.
    /// TODO(andreas): This should be a kind of factory function/builder instead which gets target memory passed in.
    pub data: Vec<u8>, // TODO: make this a slice
    pub format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    //generate_mip_maps: bool, // TODO(andreas): generate mipmaps!
}

impl Texture2DCreationDesc {
    pub fn convert_rgb8_to_rgba8(rgb_pixels: &[u8]) -> Vec<u8> {
        rgb_pixels
            .chunks_exact(3)
            .flat_map(|color| [color[0], color[1], color[2], 255])
            .collect()
    }

    /// Ensures that the data has correct row padding.
    pub fn pad_rows_if_necessary(&mut self) {
        if !self.needs_row_alignment() {
            return;
        }

        let bytes_per_row = self.bytes_per_row();

        let num_padding_bytes = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
            - (bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);

        self.data = self
            .data
            .chunks_exact(bytes_per_row as usize)
            .flat_map(|unpadded_row| {
                unpadded_row
                    .iter()
                    .cloned()
                    .chain(std::iter::repeat(255).take(num_padding_bytes as usize))
            })
            .collect();
    }

    /// Bytes per row the texture should (!) have.
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

/// Texture manager for 2D textures.
///
/// The scope is intentionally limited to particular kinds of textures that currently
/// require this kind of handle abstraction/management.
/// More complex textures types are typically handled within renderer which utilize the texture pool directly.
/// This manager in contrast, deals with user provided texture data!
/// We might revisit this later and make this texture manager more general purpose.
pub struct TextureManager2D {
    // TODO: cut out this middle man for the time being.
    manager: ResourceManager<Texture2DHandleInner, GpuTextureHandleStrong>,
    white_texture: GpuTexture2DHandle,

    // For convenience to reduce amount of times we need to pass them around
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl TextureManager2D {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        texture_pool: &mut GpuTexturePool,
    ) -> Self {
        let mut manager = ResourceManager::default();

        let white_texture = manager.store_resource(
            Self::create_and_upload_texture(
                &device,
                &queue,
                texture_pool,
                &Texture2DCreationDesc {
                    label: "placeholder".into(),
                    data: vec![255, 255, 255, 255],
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    width: 1,
                    height: 1,
                },
            ),
            ResourceLifeTime::LongLived,
        );

        Self {
            manager,
            white_texture,
            device,
            queue,
        }
    }

    /// Creates a new 2D texture resource and schedules data upload to the GPU.
    pub fn create(
        &mut self,
        texture_pool: &mut GpuTexturePool,
        mut creation_desc: Texture2DCreationDesc,
        lifetime: ResourceLifeTime,
    ) -> GpuTexture2DHandle {
        // TODO(andreas): Disabled the warning as we're moving towards using this texture manager for user-logged images.
        // However, it's still very much a concern especially once we add mipmapping. Something we need to keep in mind.
        //
        // if !resource.width.is_power_of_two() || !resource.width.is_power_of_two() {
        //     re_log::warn!(
        //         "Texture {:?} has the non-power-of-two (NPOT) resolution of {}x{}. \
        //         NPOT textures are slower and on WebGL can't handle mipmapping, UV wrapping and UV tiling",
        //         resource.label,
        //         resource.width,
        //         resource.height
        //     );
        // }
        if creation_desc.needs_row_alignment() {
            re_log::warn!(
                "Texture {:?} byte rows are not aligned to {}. Adding padding now.",
                creation_desc.label,
                wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
            );
            creation_desc.pad_rows_if_necessary();
        }

        let texture_handle = Self::create_and_upload_texture(
            &self.device,
            &self.queue,
            texture_pool,
            &creation_desc,
        );

        self.manager.store_resource(texture_handle, lifetime)
    }

    /// Returns a single pixel white pixel.
    pub fn white_texture(&self) -> &GpuTexture2DHandle {
        &self.white_texture
    }

    fn create_and_upload_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_pool: &mut GpuTexturePool,
        creation_desc: &Texture2DCreationDesc,
    ) -> GpuTextureHandleStrong {
        let size = wgpu::Extent3d {
            width: creation_desc.width,
            height: creation_desc.height,
            depth_or_array_layers: 1,
        };
        let texture_handle = texture_pool.alloc(
            device,
            &TextureDesc {
                label: creation_desc.label.clone(),
                size,
                mip_level_count: 1, // TODO(andreas)
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: creation_desc.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            },
        );
        let texture = texture_pool.get_resource(&texture_handle).unwrap();

        // TODO(andreas): temp allocator for staging data?
        // We don't do any further validation of the buffer here as wgpu does so extensively.
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &creation_desc.data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(
                    NonZeroU32::new(creation_desc.bytes_per_row()).expect("invalid bytes per row"),
                ),
                rows_per_image: None,
            },
            size,
        );

        // TODO(andreas): mipmap generation

        texture_handle
    }

    pub(crate) fn get(
        &self,
        handle: &GpuTexture2DHandle,
    ) -> Result<&GpuTextureHandleStrong, ResourceManagerError> {
        self.manager.get(handle)
    }

    pub(crate) fn frame_maintenance(&mut self, frame_index: u64) {
        self.manager.frame_maintenance(frame_index);
    }
}
