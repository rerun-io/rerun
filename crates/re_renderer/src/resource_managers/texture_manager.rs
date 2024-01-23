use std::sync::Arc;

use ahash::{HashMap, HashSet};
use parking_lot::Mutex;

use crate::{
    wgpu_resources::{GpuTexture, GpuTexturePool, TextureDesc},
    DebugLabel,
};

/// Handle to a 2D resource.
///
/// Currently, this is solely a more strongly typed regular gpu texture handle.
/// Since all textures have "long lived" behavior (no temp allocation, alive until unused),
/// there is no difference as with buffer reliant data like meshes or most contents of draw-data.
#[derive(Clone)]
pub struct GpuTexture2D(GpuTexture);

impl std::fmt::Debug for GpuTexture2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("GpuTexture2D").field(&self.0.handle).finish()
    }
}

impl GpuTexture2D {
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

/// Data required to create a texture 2d resource.
///
/// It is *not* stored along side the resulting texture resource!
pub struct Texture2DCreationDesc<'a> {
    pub label: DebugLabel,

    /// Data for the highest mipmap level.
    ///
    /// Data is expected to be tightly packed.
    /// I.e. it is *not* padded according to wgpu buffer->texture transfer rules, padding will happen on the fly if necessary.
    /// TODO(andreas): This should be a kind of factory function/builder instead which gets target memory passed in.
    pub data: std::borrow::Cow<'a, [u8]>,
    pub format: wgpu::TextureFormat,
    pub width: u32,
    pub height: u32,
    //generate_mip_maps: bool, // TODO(andreas): generate mipmaps!
}

// TODO(andreas): Move this to texture pool.
#[derive(thiserror::Error, Debug)]
pub enum TextureCreationError {
    #[error("Texture with debug label {0:?} has zero width or height!")]
    ZeroSize(DebugLabel),

    #[error("Texture was {width}x{height}, larger than the max of {max_texture_dimension_2d}")]
    TooLarge {
        width: u32,
        height: u32,
        max_texture_dimension_2d: u32,
    },

    #[error(
        "Wrong number of bytes in the texture data buffer - expected {width}x{height}x{bytes_per_texel}={expected_bytes}, got {actual_bytes}"
    )]
    WrongBufferSize {
        width: u32,
        height: u32,
        bytes_per_texel: u32,
        expected_bytes: usize, // product of the avbove

        actual_bytes: usize,
    },

    #[error(
        "Texture with debug label {label:?} has a format {format:?} that data can't be transferred to!"
    )]
    UnsupportedFormatForTransfer {
        label: DebugLabel,
        format: wgpu::TextureFormat,
    },
}

#[derive(thiserror::Error, Debug)]
pub enum TextureManager2DError<DataCreationError> {
    /// Something went wrong when creating the GPU texture.
    #[error(transparent)]
    TextureCreation(#[from] TextureCreationError),

    /// Something went wrong in a user-callback.
    #[error(transparent)]
    DataCreation(DataCreationError),
}

impl From<TextureManager2DError<never::Never>> for TextureCreationError {
    fn from(err: TextureManager2DError<never::Never>) -> Self {
        match err {
            TextureManager2DError::TextureCreation(texture_creation) => texture_creation,
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
    // Long lived/short lived doesn't make sense for textures since we don't yet know a way to
    // optimize for short lived textures as we do with buffer data.
    //manager: ResourceManager<Texture2DHandleInner, GpuTextureHandleStrong>,
    white_texture_unorm: GpuTexture2D,
    zeroed_texture_float: GpuTexture2D,
    zeroed_texture_depth: GpuTexture2D,
    zeroed_texture_sint: GpuTexture2D,
    zeroed_texture_uint: GpuTexture2D,

    // For convenience to reduce amount of times we need to pass them around
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

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
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        texture_pool: &GpuTexturePool,
    ) -> Self {
        re_tracing::profile_function!();

        let white_texture_unorm = Self::create_and_upload_texture(
            &device,
            &queue,
            texture_pool,
            &Texture2DCreationDesc {
                label: "white pixel - unorm".into(),
                data: vec![255, 255, 255, 255].into(),
                format: wgpu::TextureFormat::Rgba8Unorm,
                width: 1,
                height: 1,
            },
        )
        .expect("Failed to create white pixel texture!");

        let zeroed_texture_float =
            create_zero_texture(texture_pool, &device, wgpu::TextureFormat::Rgba8Unorm);
        let zeroed_texture_depth =
            create_zero_texture(texture_pool, &device, wgpu::TextureFormat::Depth16Unorm);
        let zeroed_texture_sint =
            create_zero_texture(texture_pool, &device, wgpu::TextureFormat::Rgba8Sint);
        let zeroed_texture_uint =
            create_zero_texture(texture_pool, &device, wgpu::TextureFormat::Rgba8Uint);

        Self {
            white_texture_unorm,
            zeroed_texture_float,
            zeroed_texture_depth,
            zeroed_texture_sint,
            zeroed_texture_uint,
            device,
            queue,
            inner: Default::default(),
        }
    }

    /// Creates a new 2D texture resource and schedules data upload to the GPU.
    /// TODO(jleibs): All usages of this should be replaced with `get_or_create`, which is strictly preferable
    pub fn create(
        &self,
        texture_pool: &GpuTexturePool,
        creation_desc: &Texture2DCreationDesc<'_>,
    ) -> Result<GpuTexture2D, TextureCreationError> {
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

        Self::create_and_upload_texture(&self.device, &self.queue, texture_pool, creation_desc)
    }

    /// Creates a new 2D texture resource and schedules data upload to the GPU if a texture
    /// wasn't already created using the same key.
    pub fn get_or_create(
        &self,
        key: u64,
        texture_pool: &GpuTexturePool,
        texture_desc: Texture2DCreationDesc<'_>,
    ) -> Result<GpuTexture2D, TextureCreationError> {
        self.get_or_create_with(key, texture_pool, || texture_desc)
    }

    /// Creates a new 2D texture resource and schedules data upload to the GPU if a texture
    /// wasn't already created using the same key.
    pub fn get_or_create_with<'a>(
        &self,
        key: u64,
        texture_pool: &GpuTexturePool,
        create_texture_desc: impl FnOnce() -> Texture2DCreationDesc<'a>,
    ) -> Result<GpuTexture2D, TextureCreationError> {
        self.get_or_try_create_with(key, texture_pool, || -> Result<_, never::Never> {
            Ok(create_texture_desc())
        })
        .map_err(|err| err.into())
    }

    /// Creates a new 2D texture resource and schedules data upload to the GPU if a texture
    /// wasn't already created using the same key.
    pub fn get_or_try_create_with<'a, Err: std::fmt::Display>(
        &self,
        key: u64,
        texture_pool: &GpuTexturePool,
        try_create_texture_desc: impl FnOnce() -> Result<Texture2DCreationDesc<'a>, Err>,
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
                let texture = Self::create_and_upload_texture(
                    &self.device,
                    &self.queue,
                    texture_pool,
                    &tex_creation_desc,
                )?;
                entry.insert(texture).clone()
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

    /// Returns a single zero pixel with format [`wgpu::TextureFormat::Depth16Unorm`].
    pub fn zeroed_texture_depth(&self) -> &GpuTexture {
        &self.zeroed_texture_depth.0
    }

    /// Returns a single zero pixel with format [`wgpu::TextureFormat::Rgba8Sint`].
    pub fn zeroed_texture_sint(&self) -> &GpuTexture {
        &self.zeroed_texture_sint.0
    }

    /// Returns a single zero pixel with format [`wgpu::TextureFormat::Rgba8Uint`].
    pub fn zeroed_texture_uint(&self) -> &GpuTexture {
        &self.zeroed_texture_uint.0
    }

    fn create_and_upload_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_pool: &GpuTexturePool,
        creation_desc: &Texture2DCreationDesc<'_>,
    ) -> Result<GpuTexture2D, TextureCreationError> {
        re_tracing::profile_function!();

        let Texture2DCreationDesc {
            label,
            data,
            format,
            width,
            height,
        } = creation_desc;
        let (width, height, format) = (*width, *height, *format);

        if width == 0 || height == 0 {
            return Err(TextureCreationError::ZeroSize(label.clone()));
        }

        let max_texture_dimension_2d = device.limits().max_texture_dimension_2d;
        if width > max_texture_dimension_2d || height > max_texture_dimension_2d {
            return Err(TextureCreationError::TooLarge {
                width,
                height,
                max_texture_dimension_2d,
            });
        }

        if !format.is_compressed() {
            if let Some(bytes_per_texel) = creation_desc
                .format
                .block_copy_size(Some(wgpu::TextureAspect::All))
            {
                let expected_bytes = width as usize * height as usize * bytes_per_texel as usize;

                if data.len() != expected_bytes {
                    return Err(TextureCreationError::WrongBufferSize {
                        width,
                        height,
                        bytes_per_texel,
                        expected_bytes,

                        actual_bytes: data.len(),
                    });
                }
            }
        }

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = texture_pool.alloc(
            device,
            &TextureDesc {
                label: label.clone(),
                size,
                mip_level_count: 1, // TODO(andreas)
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            },
        );

        let width_blocks = width / format.block_dimensions().0;
        let block_size = creation_desc
            .format
            .block_copy_size(Some(wgpu::TextureAspect::All))
            .ok_or_else(|| TextureCreationError::UnsupportedFormatForTransfer {
                label: label.clone(),
                format,
            })?;
        let bytes_per_row_unaligned = width_blocks * block_size;

        // TODO(andreas): Once we have our own temp buffer for uploading, we can do the padding inplace
        // I.e. the only difference will be if we do one memcopy or one memcopy per row, making row padding a nuisance!
        let data: &[u8] = data.as_ref();

        // TODO(andreas): temp allocator for staging data?
        re_tracing::profile_scope!("write_texture");
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
                bytes_per_row: Some(bytes_per_row_unaligned),
                rows_per_image: None,
            },
            size,
        );

        // TODO(andreas): mipmap generation

        Ok(GpuTexture2D(texture))
    }

    pub(crate) fn begin_frame(&self, _frame_index: u64) {
        self.inner.lock().begin_frame(_frame_index);
    }
}

fn create_zero_texture(
    texture_pool: &GpuTexturePool,
    device: &Arc<wgpu::Device>,
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
