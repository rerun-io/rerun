use ahash::{HashMap, HashSet};
use re_mutex::Mutex;

use super::ImageDataToTextureError;
use super::image_data_to_texture::transfer_image_data_to_texture;
use crate::RenderContext;
use crate::resource_managers::ImageDataDesc;
use crate::wgpu_resources::{GpuTexture, GpuTexturePool, TextureDesc};

/// Handle to a 2D resource.
///
/// Currently, this is solely a more strongly typed regular gpu texture handle.
#[derive(Clone)]
pub struct GpuTexture2D(GpuTexture);

impl std::fmt::Debug for GpuTexture2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GpuTexture2D").field(&self.0.handle).finish()
    }
}

impl GpuTexture2D {
    /// Returns `None` if the `texture` is not 2D.
    pub fn new(texture: GpuTexture) -> Option<Self> {
        if texture.texture.dimension() != wgpu::TextureDimension::D2 {
            return None;
        }

        Some(Self(texture))
    }

    #[inline]
    pub fn handle(&self) -> crate::wgpu_resources::GpuTextureHandle {
        self.0.handle
    }

    /// Width of the texture.
    #[inline]
    pub fn width(&self) -> u32 {
        self.0.texture.width()
    }

    /// Height of the texture.
    #[inline]
    pub fn height(&self) -> u32 {
        self.0.texture.height()
    }

    /// Width and height of the texture.
    #[inline]
    pub fn width_height(&self) -> [u32; 2] {
        [self.width(), self.height()]
    }

    #[inline]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.0.texture.format()
    }
}

impl AsRef<GpuTexture> for GpuTexture2D {
    #[inline(always)]
    fn as_ref(&self) -> &GpuTexture {
        &self.0
    }
}

impl std::ops::Deref for GpuTexture2D {
    type Target = GpuTexture;

    #[inline(always)]
    fn deref(&self) -> &GpuTexture {
        &self.0
    }
}

impl std::borrow::Borrow<GpuTexture> for GpuTexture2D {
    #[inline(always)]
    fn borrow(&self) -> &GpuTexture {
        &self.0
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TextureManager2DError<DataCreationError> {
    /// Something went wrong when creating the GPU texture & uploading/converting the image data.
    #[error(transparent)]
    ImageDataToTextureError(#[from] ImageDataToTextureError),

    /// Something went wrong in a user-callback.
    #[error(transparent)]
    DataCreation(DataCreationError),
}

impl From<TextureManager2DError<never::Never>> for ImageDataToTextureError {
    fn from(err: TextureManager2DError<never::Never>) -> Self {
        match err {
            TextureManager2DError::ImageDataToTextureError(texture_creation) => texture_creation,
            TextureManager2DError::DataCreation(never) => match never {},
        }
    }
}

/// Texture manager for 2D textures.
///
/// The scope is intentionally limited to particular kinds of textures that currently
/// require this kind of handle abstraction/management.
/// More complex textures types are typically handled within renderer which utilize the texture pool directly.
/// This manager in contrast, deals with user provided texture data!
/// We might revisit this later and make this texture manager more general purpose.
///
/// Has intertior mutability.
pub struct TextureManager2D {
    white_texture_unorm: GpuTexture2D,
    zeroed_texture_float: GpuTexture2D,
    zeroed_texture_sint: GpuTexture2D,
    zeroed_texture_uint: GpuTexture2D,

    /// The mutable part of the manager.
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// Caches textures using a unique id, which in practice is the hash of the
    /// row id of the tensor data (`tensor_data_row_id`).
    ///
    /// Any texture which wasn't accessed on the previous frame is ejected from the cache
    /// during [`Self::begin_frame`].
    texture_cache: HashMap<u64, GpuTexture2D>,

    accessed_textures: HashSet<u64>,
}

impl Inner {
    fn begin_frame(&mut self, _frame_index: u64) {
        // Drop any textures that weren't accessed in the last frame
        self.texture_cache
            .retain(|k, _| self.accessed_textures.contains(k));
        self.accessed_textures.clear();
    }
}

impl TextureManager2D {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_pool: &GpuTexturePool,
    ) -> Self {
        re_tracing::profile_function!();

        // Create the single pixel white texture ad hoc - at this point during initialization we don't have
        // the render context yet and thus can't use the higher level `transfer_image_data_to_texture` function.
        let white_texture_unorm = GpuTexture2D(texture_pool.alloc(
            device,
            &TextureDesc {
                label: "white pixel - unorm".into(),
                format: wgpu::TextureFormat::Rgba8Unorm,
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            },
        ));
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &white_texture_unorm.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let zeroed_texture_float =
            create_zero_texture(texture_pool, device, wgpu::TextureFormat::Rgba8Unorm);
        let zeroed_texture_sint =
            create_zero_texture(texture_pool, device, wgpu::TextureFormat::Rgba8Sint);
        let zeroed_texture_uint =
            create_zero_texture(texture_pool, device, wgpu::TextureFormat::Rgba8Uint);

        Self {
            white_texture_unorm,
            zeroed_texture_float,
            zeroed_texture_sint,
            zeroed_texture_uint,
            inner: Default::default(),
        }
    }

    /// Creates a new 2D texture resource and schedules data upload to the GPU.
    /// TODO(jleibs): All usages of this should be replaced with `get_or_create`, which is strictly preferable
    #[expect(clippy::unused_self)]
    pub fn create(
        &self,
        render_ctx: &RenderContext,
        creation_desc: ImageDataDesc<'_>,
    ) -> Result<GpuTexture2D, ImageDataToTextureError> {
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

        // Currently we don't store any data in the texture manager.
        // In the future we might handle (lazy?) mipmap generation in here or keep track of lazy upload processing.

        let texture =
            creation_desc.create_target_texture(render_ctx, wgpu::TextureUsages::TEXTURE_BINDING);
        transfer_image_data_to_texture(render_ctx, creation_desc, &texture)?;
        Ok(GpuTexture2D(texture))
    }

    /// Creates a new 2D texture resource and schedules data upload to the GPU if a texture
    /// wasn't already created using the same key.
    pub fn get_or_create(
        &self,
        key: u64,
        render_ctx: &RenderContext,
        texture_desc: ImageDataDesc<'_>,
    ) -> Result<GpuTexture2D, ImageDataToTextureError> {
        self.get_or_create_with(key, render_ctx, || texture_desc)
    }

    /// Creates a new 2D texture resource and schedules data upload to the GPU if a texture
    /// wasn't already created using the same key.
    pub fn get_or_create_with<'a>(
        &self,
        key: u64,
        render_ctx: &RenderContext,
        create_texture_desc: impl FnOnce() -> ImageDataDesc<'a>,
    ) -> Result<GpuTexture2D, ImageDataToTextureError> {
        self.get_or_try_create_with(key, render_ctx, || -> Result<_, never::Never> {
            Ok(create_texture_desc())
        })
        .map_err(|err| err.into())
    }

    /// Creates a new 2D texture resource and schedules data upload to the GPU if a texture
    /// wasn't already created using the same key.
    pub fn get_or_try_create_with<'a, Err: std::fmt::Display>(
        &self,
        key: u64,
        render_ctx: &RenderContext,
        try_create_texture_desc: impl FnOnce() -> Result<ImageDataDesc<'a>, Err>,
    ) -> Result<GpuTexture2D, TextureManager2DError<Err>> {
        let mut inner = self.inner.lock();
        let texture_handle = match inner.texture_cache.entry(key) {
            std::collections::hash_map::Entry::Occupied(texture_handle) => {
                texture_handle.get().clone() // already inserted
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                // Run potentially expensive texture creation code:
                let tex_creation_desc = try_create_texture_desc()
                    .map_err(|err| TextureManager2DError::DataCreation(err))?;

                let texture = tex_creation_desc
                    .create_target_texture(render_ctx, wgpu::TextureUsages::TEXTURE_BINDING);
                transfer_image_data_to_texture(render_ctx, tex_creation_desc, &texture)?;
                entry.insert(GpuTexture2D(texture)).clone()
            }
        };

        inner.accessed_textures.insert(key);
        Ok(texture_handle)
    }

    /// Returns a single pixel white pixel with an rgba8unorm format.
    pub fn white_texture_unorm_handle(&self) -> &GpuTexture2D {
        &self.white_texture_unorm
    }

    /// Returns a single pixel white pixel with an rgba8unorm format.
    pub fn white_texture_unorm(&self) -> &GpuTexture {
        &self.white_texture_unorm.0
    }

    /// Returns a single zero pixel with format [`wgpu::TextureFormat::Rgba8Unorm`].
    pub fn zeroed_texture_float(&self) -> &GpuTexture {
        &self.zeroed_texture_float.0
    }

    /// Returns a single zero pixel with format [`wgpu::TextureFormat::Rgba8Sint`].
    pub fn zeroed_texture_sint(&self) -> &GpuTexture {
        &self.zeroed_texture_sint.0
    }

    /// Returns a single zero pixel with format [`wgpu::TextureFormat::Rgba8Uint`].
    pub fn zeroed_texture_uint(&self) -> &GpuTexture {
        &self.zeroed_texture_uint.0
    }

    pub(crate) fn begin_frame(&self, _frame_index: u64) {
        self.inner.lock().begin_frame(_frame_index);
    }
}

fn create_zero_texture(
    texture_pool: &GpuTexturePool,
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> GpuTexture2D {
    // Wgpu zeros out new textures automatically
    GpuTexture2D(texture_pool.alloc(
        device,
        &TextureDesc {
            label: format!("zeroed pixel {format:?}").into(),
            format,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
        },
    ))
}
