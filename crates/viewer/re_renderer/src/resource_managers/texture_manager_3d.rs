use ahash::{HashMap, HashSet};
use re_mutex::Mutex;

use crate::RenderContext;
use crate::wgpu_resources::{GpuTexture, TextureDesc};

/// Handle to a 3D texture resource.
///
/// Used for volumetric data such as voxel grids, CT scans, etc.
#[derive(Clone)]
pub struct GpuTexture3D {
    texture: GpuTexture,
}

impl std::fmt::Debug for GpuTexture3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuTexture3D")
            .field("handle", &self.texture.handle)
            .finish()
    }
}

impl GpuTexture3D {
    /// Returns `None` if the `texture` is not 3D.
    pub fn new(texture: GpuTexture) -> Option<Self> {
        if texture.texture.dimension() != wgpu::TextureDimension::D3 {
            return None;
        }
        Some(Self { texture })
    }

    #[inline]
    pub fn handle(&self) -> crate::wgpu_resources::GpuTextureHandle {
        self.texture.handle
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.texture.texture.width()
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.texture.texture.height()
    }

    #[inline]
    pub fn depth(&self) -> u32 {
        self.texture.texture.depth_or_array_layers()
    }

    #[inline]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.texture.texture.format()
    }
}

impl AsRef<GpuTexture> for GpuTexture3D {
    #[inline(always)]
    fn as_ref(&self) -> &GpuTexture {
        &self.texture
    }
}

impl std::ops::Deref for GpuTexture3D {
    type Target = GpuTexture;

    #[inline(always)]
    fn deref(&self) -> &GpuTexture {
        &self.texture
    }
}

impl std::borrow::Borrow<GpuTexture> for GpuTexture3D {
    #[inline(always)]
    fn borrow(&self) -> &GpuTexture {
        &self.texture
    }
}

/// Texture manager for 3D textures (volumetric data).
///
/// Manages upload and caching of 3D textures from flat voxel data.
/// Textures not accessed in the previous frame are evicted.
///
/// Has interior mutability.
pub struct TextureManager3D {
    inner: Mutex<Inner3D>,
}

#[derive(Default)]
struct Inner3D {
    texture_cache: HashMap<u64, GpuTexture3D>,
    accessed_textures: HashSet<u64>,
}

impl Inner3D {
    fn begin_frame(&mut self) {
        self.texture_cache
            .retain(|k, _| self.accessed_textures.contains(k));
        self.accessed_textures.clear();
    }
}

/// Description of 3D volume data to upload as a GPU texture.
pub struct VolumeDataDesc<'a> {
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub format: wgpu::TextureFormat,
    pub data: &'a [u8],
}

impl TextureManager3D {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    /// Creates a new 3D texture from volume data, or retrieves it from cache.
    pub fn get_or_create(
        &self,
        key: u64,
        render_ctx: &RenderContext,
        desc: VolumeDataDesc<'_>,
    ) -> GpuTexture3D {
        let mut inner = self.inner.lock();

        let texture = match inner.texture_cache.entry(key) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let gpu_texture = Self::upload_volume(render_ctx, &desc);
                entry.insert(gpu_texture).clone()
            }
        };

        inner.accessed_textures.insert(key);
        texture
    }

    fn upload_volume(render_ctx: &RenderContext, desc: &VolumeDataDesc<'_>) -> GpuTexture3D {
        re_tracing::profile_function!();

        let bytes_per_texel = desc.format.block_copy_size(None).unwrap_or(4);
        let expected_size = desc.width as usize
            * desc.height as usize
            * desc.depth as usize
            * bytes_per_texel as usize;

        re_log::debug_assert_eq!(
            desc.data.len(),
            expected_size,
            "Volume data size mismatch: expected {expected_size} bytes for {}x{}x{} {:?}, got {}",
            desc.width,
            desc.height,
            desc.depth,
            desc.format,
            desc.data.len()
        );

        let texture = render_ctx.gpu_resources.textures.alloc(
            &render_ctx.device,
            &TextureDesc {
                label: desc.label.clone().into(),
                format: desc.format,
                size: wgpu::Extent3d {
                    width: desc.width,
                    height: desc.height,
                    depth_or_array_layers: desc.depth,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            },
        );

        let bytes_per_row = bytes_per_texel * desc.width;
        let rows_per_image = desc.height;

        render_ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            desc.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(rows_per_image),
            },
            wgpu::Extent3d {
                width: desc.width,
                height: desc.height,
                depth_or_array_layers: desc.depth,
            },
        );

        GpuTexture3D::new(texture).expect("Texture is known to be 3D")
    }

    pub(crate) fn begin_frame(&self) {
        self.inner.lock().begin_frame();
    }
}

impl Default for TextureManager3D {
    fn default() -> Self {
        Self::new()
    }
}
