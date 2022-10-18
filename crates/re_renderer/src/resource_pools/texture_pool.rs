use std::{hash::Hash, sync::atomic::AtomicU64};

use super::resource_pool::*;

slotmap::new_key_type! { pub(crate) struct TextureHandle; }

pub(crate) struct Texture {
    usage_state: AtomicU64,
    pub(crate) _texture: wgpu::Texture,
    pub(crate) default_view: wgpu::TextureView,
    // TODO(andreas) what about custom views
}

impl UsageTrackedResource for Texture {
    fn usage_state(&self) -> &AtomicU64 {
        &self.usage_state
    }
}

// TODO(andreas) use a custom descriptor type with [`DebugLabel`] and a content id.
type TextureDesc = wgpu::TextureDescriptor<'static>;

#[derive(Default)]
pub(crate) struct TexturePool {
    pool: ResourcePool<TextureHandle, TextureDesc, Texture>,
}

impl TexturePool {
    pub fn request(&mut self, device: &wgpu::Device, desc: &TextureDesc) -> TextureHandle {
        self.pool.get_handle(desc, |desc| {
            let texture = device.create_texture(desc);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            Texture {
                usage_state: AtomicU64::new(0),
                _texture: texture,
                default_view: view,
            }
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool.discard_unused_resources(frame_index);
    }
}

pub(crate) fn render_target_2d_desc(
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    sample_count: u32,
) -> TextureDesc {
    wgpu::TextureDescriptor {
        label: Some("rendertarget"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    }
}

impl<'a> ResourcePoolFacade<'a, TextureHandle, TextureDesc, Texture> for TexturePool {
    fn pool(&'a self) -> &ResourcePool<TextureHandle, TextureDesc, Texture> {
        &self.pool
    }
}
