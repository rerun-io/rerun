use std::borrow::Cow;

use slotmap::SlotMap;

use crate::resource_pools::texture_pool::GpuTextureHandleStrong;

slotmap::new_key_type! { pub struct TextureStoreHandle; }

pub struct Texture {
    data: Cow<[u8]>,
    format: wgpu::TextureFormat,
    dimension: wgpu::TextureDimension,
    size: wgpu::Extent3d,
}

enum TextureStoreEntry {
    CpuData(Texture),
    GpuData(GpuTextureHandleStrong),
}



/// Simple texture manager facilitating texture data reuse and lazy upload of texture data.
struct TextureStore {
    long_lived_meshes: SlotMap<DefaultKey, GpuMesh>,
    frame_meshes: SlotMap<DefaultKey, GpuMesh>,
}

impl TextureStore {
    pub fn create_texture_single_pixel_texture(&mut self, color_srgba: [u8; 4]) -> TextureStoreHandle {
        self.create_texture(Texture {
            dimension: wgpu::TextureDimension::D2,
            data: Cow::from(&color_srgba[..]),
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        })
    }

    pub fn create_texture(&mut self, texture: Texture) -> TextureStoreHandle {
        self.gpu_textures.insert(TextureStoreEntry::CpuData(texture))
    }


    pub(crate) gpu_texture(&mut self, handle: TextureHandleManaged,
        device: &wgpu::Device, queue: &wgpu::Queue,
        texture_pool: &mut TexturePool) -> GpuTextureHandleStrong {

    }
}
