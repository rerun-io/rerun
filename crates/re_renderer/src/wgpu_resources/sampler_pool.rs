use std::hash::Hash;

use super::static_resource_pool::{StaticResourcePool, StaticResourcePoolReadLockAccessor};
use crate::debug_label::DebugLabel;

slotmap::new_key_type! { pub struct GpuSamplerHandle; }

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct SamplerDesc {
    /// Debug label of the sampler. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    /// How to deal with out of bounds accesses in the u (i.e. x) direction
    pub address_mode_u: wgpu::AddressMode,

    /// How to deal with out of bounds accesses in the v (i.e. y) direction
    pub address_mode_v: wgpu::AddressMode,

    /// How to deal with out of bounds accesses in the w (i.e. z) direction
    pub address_mode_w: wgpu::AddressMode,

    /// How to filter the texture when it needs to be magnified (made larger)
    pub mag_filter: wgpu::FilterMode,

    /// How to filter the texture when it needs to be minified (made smaller)
    pub min_filter: wgpu::FilterMode,

    /// How to filter between mip map levels
    pub mipmap_filter: wgpu::FilterMode,

    /// Minimum level of detail (i.e. mip level) to use
    pub lod_min_clamp: ordered_float::NotNan<f32>,

    /// Maximum level of detail (i.e. mip level) to use
    pub lod_max_clamp: ordered_float::NotNan<f32>,
}

#[derive(Default)]
pub struct GpuSamplerPool {
    pool: StaticResourcePool<GpuSamplerHandle, SamplerDesc, wgpu::Sampler>,
}

impl GpuSamplerPool {
    pub fn get_or_create(&self, device: &wgpu::Device, desc: &SamplerDesc) -> GpuSamplerHandle {
        self.pool.get_or_create(desc, |desc| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: desc.label.get(),
                address_mode_u: desc.address_mode_u,
                address_mode_v: desc.address_mode_v,
                address_mode_w: desc.address_mode_w,
                mag_filter: desc.mag_filter,
                min_filter: desc.min_filter,
                mipmap_filter: desc.mipmap_filter,
                lod_min_clamp: desc.lod_min_clamp.into(),
                lod_max_clamp: desc.lod_max_clamp.into(),

                // Unsupported
                compare: None,
                border_color: None,
                anisotropy_clamp: 1,
            })
        })
    }

    /// Locks the resource pool for resolving handles.
    ///
    /// While it is locked, no new resources can be added.
    pub fn resources(
        &self,
    ) -> StaticResourcePoolReadLockAccessor<'_, GpuSamplerHandle, wgpu::Sampler> {
        self.pool.resources()
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        self.pool.current_frame_index = frame_index;
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }
}
