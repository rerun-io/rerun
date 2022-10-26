use std::{hash::Hash, sync::atomic::AtomicU64};

use super::{resource::*, static_resource_pool::*};

slotmap::new_key_type! { pub struct TextureHandle; }

pub(crate) struct Texture {
    last_frame_used: AtomicU64,
    pub(crate) texture: wgpu::Texture,
    pub(crate) default_view: wgpu::TextureView,
    // TODO(andreas) what about custom views
}

impl UsageTrackedResource for Texture {
    fn last_frame_used(&self) -> &AtomicU64 {
        &self.last_frame_used
    }
}

// TODO(andreas) use a custom descriptor type with [`DebugLabel`] and a content id.
#[derive(Default)]
pub(crate) struct TexturePool {
    pool: StaticResourcePool<TextureHandle, wgpu::TextureDescriptor<'static>, Texture>,
}

impl TexturePool {
    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &wgpu::TextureDescriptor<'static>,
    ) -> TextureHandle {
        self.pool.get_or_create(desc, |desc| {
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
        self.pool.discard_unused_resources(frame_index);
    }

    pub fn get(&self, handle: TextureHandle) -> Result<&Texture, PoolError> {
        self.pool.get_resource(handle)
    }

    pub(super) fn register_resource_usage(&mut self, handle: TextureHandle) {
        let _ = self.get(handle);
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
