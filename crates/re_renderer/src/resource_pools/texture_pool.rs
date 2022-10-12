use slotmap::new_key_type;
use std::{
    hash::Hash,
    sync::atomic::{AtomicU64, Ordering},
};

use super::resource_pool::*;

new_key_type! { pub(crate) struct TextureHandle; }

pub(crate) struct Texture {
    last_frame_used: AtomicU64,
    pub(crate) texture: wgpu::Texture,
    pub(crate) default_view: wgpu::TextureView,
    // TODO(andreas) what about custom views
}

impl Resource for Texture {
    fn register_use(&self, current_frame_index: u64) {
        self.last_frame_used
            .fetch_max(current_frame_index, Ordering::Relaxed);
    }
}

pub(crate) struct TexturePool {
    // TODO(andreas): Ignore label for hashing/comparing?
    pool: ResourcePool<TextureHandle, wgpu::TextureDescriptor<'static>, Texture>,
}

impl TexturePool {
    pub fn new() -> Self {
        TexturePool {
            pool: ResourcePool::new(),
        }
    }

    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &wgpu::TextureDescriptor<'static>,
    ) -> TextureHandle {
        self.pool.request(desc, |desc| {
            let texture = device.create_texture(desc);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            Texture {
                last_frame_used: AtomicU64::new(0),
                texture,
                default_view: view,
            }
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool.frame_maintenance(frame_index);
    }

    pub fn get(&self, handle: TextureHandle) -> Result<&Texture, PoolError> {
        self.pool.get(handle)
    }
}

pub(crate) fn render_target_2d_desc(
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    sample_count: u32,
) -> wgpu::TextureDescriptor<'static> {
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
