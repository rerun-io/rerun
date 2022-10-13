use std::{
    hash::{Hash, Hasher},
    num::NonZeroU8,
};

use super::resource_pool::*;

slotmap::new_key_type! { pub(crate) struct SamplerHandle; }

pub(crate) struct Sampler {
    pub(crate) sampler: wgpu::Sampler,
}

impl Resource for Sampler {}

#[derive(Clone)]
pub(crate) struct SamplerDesc {
    /// Debug label of the sampler. This will show up in graphics debuggers for easy identification.
    pub label: String,
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
    pub lod_min_clamp: f32,
    /// Maximum level of detail (i.e. mip level) to use
    pub lod_max_clamp: f32,
    /// Valid values: 1, 2, 4, 8, and 16.
    pub anisotropy_clamp: Option<NonZeroU8>,
}

impl Hash for SamplerDesc {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.address_mode_u.hash(state);
        self.address_mode_v.hash(state);
        self.address_mode_w.hash(state);
        self.mag_filter.hash(state);
        self.min_filter.hash(state);
        self.mipmap_filter.hash(state);
        self.lod_min_clamp.to_bits().hash(state);
        self.lod_max_clamp.to_bits().hash(state);
        self.anisotropy_clamp.hash(state);
        self.label.hash(state);
    }
}

impl PartialEq for SamplerDesc {
    fn eq(&self, other: &Self) -> bool {
        self.address_mode_u == other.address_mode_u
            && self.address_mode_v == other.address_mode_v
            && self.address_mode_w == other.address_mode_w
            && self.mag_filter == other.mag_filter
            && self.min_filter == other.min_filter
            && self.mipmap_filter == other.mipmap_filter
            && self.lod_min_clamp.to_bits() == other.lod_min_clamp.to_bits()
            && self.lod_max_clamp.to_bits() == other.lod_max_clamp.to_bits()
            && self.anisotropy_clamp == other.anisotropy_clamp
            && self.label == other.label
    }
}

impl Default for SamplerDesc {
    fn default() -> Self {
        Self {
            label: "[UNNAMED]".to_owned(),
            address_mode_u: Default::default(),
            address_mode_v: Default::default(),
            address_mode_w: Default::default(),
            mag_filter: Default::default(),
            min_filter: Default::default(),
            mipmap_filter: Default::default(),
            lod_min_clamp: 0.0,
            lod_max_clamp: std::f32::MAX,
            anisotropy_clamp: None,
        }
    }
}

impl Eq for SamplerDesc {}

pub(crate) struct SamplerPool {
    pool: ResourcePool<SamplerHandle, SamplerDesc, Sampler>,
}

impl SamplerPool {
    pub fn new() -> Self {
        SamplerPool {
            pool: ResourcePool::new(),
        }
    }

    pub fn request(&mut self, device: &wgpu::Device, desc: &SamplerDesc) -> SamplerHandle {
        self.pool.request(desc, |desc| {
            // TODO(andreas): error handling
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(&desc.label),
                address_mode_u: desc.address_mode_u,
                address_mode_v: desc.address_mode_v,
                address_mode_w: desc.address_mode_w,
                mag_filter: desc.mag_filter,
                min_filter: desc.min_filter,
                mipmap_filter: desc.mipmap_filter,
                lod_min_clamp: desc.lod_min_clamp,
                lod_max_clamp: desc.lod_max_clamp,
                anisotropy_clamp: desc.anisotropy_clamp,

                // Unsupported
                compare: None,
                border_color: None,
            });
            Sampler { sampler }
        })
    }

    pub fn get(&self, handle: SamplerHandle) -> Result<&Sampler, PoolError> {
        self.pool.get(handle)
    }
}
