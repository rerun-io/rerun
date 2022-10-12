use slotmap::{new_key_type, Key, SlotMap};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::pool_error::PoolError;

new_key_type! { pub(crate) struct TextureHandle; }

pub(crate) struct Texture {
    last_frame_used: AtomicU64,
    pub(crate) texture: wgpu::Texture,
    pub(crate) default_view: wgpu::TextureView,
    // TODO(andreas) what about custom views
}

pub(crate) struct TexturePool {
    textures: SlotMap<TextureHandle, Texture>,
    texture_lookup: HashMap<wgpu::TextureDescriptor<'static>, TextureHandle>,
    current_frame_index: u64,
}

impl TexturePool {
    pub fn new() -> Self {
        TexturePool {
            textures: SlotMap::with_key(),
            texture_lookup: HashMap::new(),
            current_frame_index: 0,
        }
    }

    pub fn request_texture(
        &mut self,
        device: &wgpu::Device,
        desc: &wgpu::TextureDescriptor<'static>,
    ) -> TextureHandle {
        *self.texture_lookup.entry(desc.clone()).or_insert_with(|| {
            let texture = device.create_texture(desc);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.textures.insert(Texture {
                last_frame_used: AtomicU64::new(0),
                texture,
                default_view: view,
            })
        })
    }

    // TODO: Make this a descriptor generator
    pub fn request_2d_render_target(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        sample_count: u32,
    ) -> TextureHandle {
        self.request_texture(
            device,
            &wgpu::TextureDescriptor {
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
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
            },
        )
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        // TODO: Remove texture that we haven't used for a while.
        self.current_frame_index = frame_index;
    }

    pub fn texture(&self, handle: TextureHandle) -> Result<&Texture, PoolError> {
        self.textures
            .get(handle)
            .map(|texture| {
                texture
                    .last_frame_used
                    .fetch_max(self.current_frame_index, Ordering::Relaxed);
                texture
            })
            .ok_or_else(|| {
                if handle.is_null() {
                    PoolError::NullHandle
                } else {
                    PoolError::ResourceNotAvailable
                }
            })
    }
}
