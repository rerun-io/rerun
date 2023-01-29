use std::{num::NonZeroU32, sync::Arc};

use glam::UVec3;

use crate::{
    wgpu_resources::{GpuTextureHandleStrong, GpuTexturePool, TextureDesc},
    DebugLabel,
};

use super::ResourceManagerError;

// --- 2D ---

/// Handle to a 2D resource.
///
/// Currently, this is solely a more strongly typed regular gpu texture handle.
/// Since all textures have "long lived" behavior (no temp allocation, alive until unused),
/// there is no difference as with buffer reliant data like meshes or most contents of draw-data.
#[derive(Clone)]
pub struct GpuTexture2DHandle(GpuTextureHandleStrong);

impl GpuTexture2DHandle {
    pub fn invalid() -> Self {
        Self(Arc::new(crate::wgpu_resources::GpuTextureHandle::default()))
    }
}

/// Data required to create a texture 2d resource.
///
/// It is *not* stored along side the resulting texture resource!
pub struct Texture2DCreationDesc<'a> {
    pub label: DebugLabel,

    /// Data for the highest mipmap level.
    /// Must be padded according to wgpu rules and ready for upload.
    /// TODO(andreas): This should be a kind of factory function/builder instead which gets target memory passed in.
    pub data: &'a [u8],
    pub format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    //generate_mip_maps: bool, // TODO(andreas): generate mipmaps!
}

impl<'a> Texture2DCreationDesc<'a> {
    pub fn convert_rgb8_to_rgba8(rgb_pixels: &[u8]) -> Vec<u8> {
        rgb_pixels
            .chunks_exact(3)
            .flat_map(|color| [color[0], color[1], color[2], 255])
            .collect()
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
    // Long lived/short lived doesn't make sense for textures since we don't yet know a way to
    // optimize for short lived textures as we do with buffer data.
    //manager: ResourceManager<Texture2DHandleInner, GpuTextureHandleStrong>,
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
        let white_texture = Self::create_and_upload_texture(
            &device,
            &queue,
            texture_pool,
            &Texture2DCreationDesc {
                label: "placeholder".into(),
                data: &[255, 255, 255, 255],
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                width: 1,
                height: 1,
            },
        );

        Self {
            white_texture,
            device,
            queue,
        }
    }

    /// Creates a new 2D texture resource and schedules data upload to the GPU.
    #[allow(clippy::unused_self)]
    pub fn create(
        &mut self,
        texture_pool: &mut GpuTexturePool,
        creation_desc: &Texture2DCreationDesc<'_>,
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

        // Currently we don't store any data in the the texture manager.
        // In the future we might handle (lazy?) mipmap generation in here or keep track of lazy upload processing.

        Self::create_and_upload_texture(&self.device, &self.queue, texture_pool, creation_desc)
    }

    /// Returns a single pixel white pixel.
    pub fn white_texture_handle(&self) -> &GpuTexture2DHandle {
        &self.white_texture
    }

    fn create_and_upload_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_pool: &mut GpuTexturePool,
        creation_desc: &Texture2DCreationDesc<'_>,
    ) -> GpuTexture2DHandle {
        crate::profile_function!();
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

        let format_info = creation_desc.format.describe();
        let width_blocks = creation_desc.width / format_info.block_dimensions.0 as u32;
        let bytes_per_row_unaligned = width_blocks * format_info.block_size as u32;

        // TODO(andreas): Once we have our own temp buffer for uploading, we can do the padding inplace
        // I.e. the only difference will be if we do one memcopy or one memcopy per row, making row padding a nuissance!
        let data = creation_desc.data;

        // TODO(andreas): temp allocator for staging data?
        // We don't do any further validation of the buffer here as wgpu does so extensively.
        crate::profile_scope!("write_texture");
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(
                    NonZeroU32::new(bytes_per_row_unaligned).expect("invalid bytes per row"),
                ),
                rows_per_image: None,
            },
            size,
        );

        // TODO(andreas): mipmap generation

        GpuTexture2DHandle(texture_handle)
    }

    /// Retrieves gpu handle.
    ///
    /// TODO(andreas): Lifetime dependency from incoming and returned handle will likely be removed in the future.
    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    pub(crate) fn get<'a>(
        &self,
        handle: &'a GpuTexture2DHandle,
    ) -> Result<&'a GpuTextureHandleStrong, ResourceManagerError> {
        Ok(&handle.0)
    }

    #[allow(clippy::unused_self)]
    pub(crate) fn frame_maintenance(&mut self, _frame_index: u64) {
        // no-op.
        // In the future we might add handling of background processing or introduce frame-lived textures.
    }
}

// --- 3D ---

// TODO: short-lived might be more important than for the 2D case here.

/// Handle to a 3D resource.
///
/// Currently, this is solely a more strongly typed regular gpu texture handle.
/// Since all textures have "long lived" behavior (no temp allocation, alive until unused),
/// there is no difference as with buffer reliant data like meshes or most contents of draw-data.
#[derive(Clone)]
pub struct GpuTexture3DHandle(GpuTextureHandleStrong);

impl GpuTexture3DHandle {
    pub fn invalid() -> Self {
        Self(Arc::new(crate::wgpu_resources::GpuTextureHandle::default()))
    }
}

/// Data required to create a texture 3D resource.
///
/// It is *not* stored along side the resulting texture resource!
pub struct Texture3DCreationDesc<'a> {
    pub label: DebugLabel,
    /// Data for the highest mipmap level.
    /// Must be padded according to wgpu rules and ready for upload.
    /// TODO(andreas): This should be a kind of factory function/builder instead which gets target memory passed in.
    pub data: &'a [u8],
    pub format: wgpu::TextureFormat,
    pub dimensions: UVec3,
    //generate_mip_maps: bool, // TODO(andreas): generate mipmaps!
}

impl<'a> Texture3DCreationDesc<'a> {
    pub fn convert_rgb8_to_rgba8(rgb_pixels: &[u8]) -> Vec<u8> {
        rgb_pixels
            .chunks_exact(3)
            .flat_map(|color| [color[0], color[1], color[2], 255])
            .collect()
    }
}

/// Texture manager for 3D textures.
///
/// The scope is intentionally limited to particular kinds of textures that currently
/// require this kind of handle abstraction/management.
/// More complex textures types are typically handled within renderer which utilize the texture pool directly.
/// This manager in contrast, deals with user provided texture data!
/// We might revisit this later and make this texture manager more general purpose.
pub struct TextureManager3D {
    // Long lived/short lived doesn't make sense for textures since we don't yet know a way to
    // optimize for short lived textures as we do with buffer data.
    // TODO
    //manager: ResourceManager<Texture3DHandleInner, GpuTextureHandleStrong>,
    white_texture: GpuTexture3DHandle,

    // For convenience to reduce amount of times we need to pass them around
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl TextureManager3D {
    pub(crate) fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        texture_pool: &mut GpuTexturePool,
    ) -> Self {
        let white_texture = Self::create_and_upload_texture(
            &device,
            &queue,
            texture_pool,
            &Texture3DCreationDesc {
                label: "placeholder".into(),
                data: &[255, 255, 255, 255],
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                dimensions: UVec3::ONE,
            },
        );

        Self {
            white_texture,
            device,
            queue,
        }
    }

    /// Creates a new 3D texture resource and schedules data upload to the GPU.
    #[allow(clippy::unused_self)]
    pub fn create(
        &mut self,
        texture_pool: &mut GpuTexturePool,
        creation_desc: &Texture3DCreationDesc<'_>,
    ) -> GpuTexture3DHandle {
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

        // Currently we don't store any data in the the texture manager.
        // In the future we might handle (lazy?) mipmap generation in here or keep track of lazy upload processing.

        Self::create_and_upload_texture(&self.device, &self.queue, texture_pool, creation_desc)
    }

    /// Returns a white unit cube.
    pub fn white_texture_handle(&self) -> &GpuTexture3DHandle {
        &self.white_texture
    }

    fn create_and_upload_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_pool: &mut GpuTexturePool,
        creation_desc: &Texture3DCreationDesc<'_>,
    ) -> GpuTexture3DHandle {
        crate::profile_function!();
        let size = wgpu::Extent3d {
            width: creation_desc.dimensions.x,
            height: creation_desc.dimensions.y,
            depth_or_array_layers: creation_desc.dimensions.z,
        };
        let texture_handle = texture_pool.alloc(
            device,
            &TextureDesc {
                label: creation_desc.label.clone(),
                size,
                mip_level_count: 1, // TODO(andreas)
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format: creation_desc.format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            },
        );
        let texture = texture_pool.get_resource(&texture_handle).unwrap();

        let format_info = creation_desc.format.describe();
        let width_blocks = creation_desc.dimensions.x / format_info.block_dimensions.0 as u32;
        let bytes_per_row_unaligned = width_blocks * format_info.block_size as u32;

        // TODO(andreas): Once we have our own temp buffer for uploading, we can do the padding inplace
        // I.e. the only difference will be if we do one memcopy or one memcopy per row, making row padding a nuissance!
        let data = creation_desc.data;

        // TODO(andreas): temp allocator for staging data?
        // We don't do any further validation of the buffer here as wgpu does so extensively.
        crate::profile_scope!("write_texture");
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(
                    NonZeroU32::new(bytes_per_row_unaligned).expect("invalid bytes per row"),
                ),
                rows_per_image: Some(creation_desc.dimensions.y.try_into().unwrap()),
            },
            size,
        );

        // TODO(andreas): mipmap generation

        GpuTexture3DHandle(texture_handle)
    }

    /// Retrieves gpu handle.
    ///
    /// TODO(andreas): Lifetime dependency from incoming and returned handle will likely be removed in the future.
    #[allow(clippy::unnecessary_wraps, clippy::unused_self)]
    pub(crate) fn get<'a>(
        &self,
        handle: &'a GpuTexture3DHandle,
    ) -> Result<&'a GpuTextureHandleStrong, ResourceManagerError> {
        Ok(&handle.0)
    }

    #[allow(clippy::unused_self)]
    pub(crate) fn frame_maintenance(&mut self, _frame_index: u64) {
        // no-op.
        // In the future we might add handling of background processing or introduce frame-lived textures.
    }
}
