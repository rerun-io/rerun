use std::{hash::Hash, sync::atomic::AtomicU64};

use crate::debug_label::DebugLabel;

use super::{dynamic_resource_pool::DynamicResourcePool, resource::*};

slotmap::new_key_type! { pub struct TextureHandle; }
pub type TextureHandleStrong = std::sync::Arc<TextureHandle>;

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

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct TextureDesc {
    /// Debug label of the texture. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    /// Size of the texture. All components must be greater than zero. For a
    /// regular 1D/2D texture, the unused sizes will be 1. For 2DArray textures,
    /// Z is the number of 2D textures in that array.
    pub size: wgpu::Extent3d,

    /// Mip count of texture. For a texture with no extra mips, this must be 1.
    pub mip_level_count: u32,

    /// Sample count of texture. If this is not 1, texture must have [`BindingType::Texture::multisampled`] set to true.
    pub sample_count: u32,

    /// Dimensions of the texture.
    pub dimension: wgpu::TextureDimension,

    /// Format of the texture.
    pub format: wgpu::TextureFormat,

    /// Allowed usages of the texture. If used in other ways, the operation will panic.
    pub usage: wgpu::TextureUsages,
}

impl TextureDesc {
    fn to_wgpu_desc<'a>(&'a self) -> wgpu::TextureDescriptor<'a> {
        wgpu::TextureDescriptor {
            label: self.label.get(),
            size: self.size,
            mip_level_count: self.mip_level_count,
            sample_count: self.sample_count,
            dimension: self.dimension,
            format: self.format,
            usage: self.usage,
        }
    }
}

#[derive(Default)]
pub(crate) struct TexturePool {
    pool: DynamicResourcePool<TextureHandle, TextureDesc, Texture>,
}

impl TexturePool {
    pub fn alloc(
        &mut self,
        device: &wgpu::Device,
        desc: &TextureDesc,
    ) -> anyhow::Result<TextureHandleStrong> {
        self.pool.alloc(desc, |desc| {
            let texture = device.create_texture(&desc.to_wgpu_desc());
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            Ok(Texture {
                last_frame_used: AtomicU64::new(0),
                texture,
                default_view: view,
            })
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool.frame_maintenance(frame_index);
    }

    /// Takes strong buffer handle to ensure the user is still holding on to the buffer.
    pub fn get_resource(&self, handle: &TextureHandleStrong) -> Result<&Texture, PoolError> {
        self.pool.get_resource(**handle)
    }

    /// Internal method to retrieve a resource with a weak handle (used by [`BindGroupPool`])
    pub(super) fn get_resource_weak(&self, handle: TextureHandle) -> Result<&Texture, PoolError> {
        self.pool.get_resource(handle)
    }

    /// Internal method to retrieve a strong handle from a weak handle (used by [`BindGroupPool`])
    pub(super) fn get_strong_handle(&self, handle: TextureHandle) -> &TextureHandleStrong {
        self.pool.get_strong_handle(handle)
    }
}

pub(crate) fn render_target_2d_desc(
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    sample_count: u32,
) -> TextureDesc {
    TextureDesc {
        label: "rendertarget".into(),
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
