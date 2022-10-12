use slotmap::new_key_type;

use super::{
    bind_group_layout_pool::{BindGroupLayoutHandle, BindGroupLayoutPool},
    resource_pool::*,
    sampler_pool::{SamplerHandle, SamplerPool},
    texture_pool::{TextureHandle, TexturePool},
};

new_key_type! { pub(crate) struct BindGroupHandle; }

pub(crate) struct BindGroup {
    pub(crate) bind_group: wgpu::BindGroup,
}

impl Resource for BindGroup {
    fn register_use(&self, _current_frame_index: u64) {
        // TODO(andreas): When a bind group  is last used doesn't tell us all that much since it's needed for pipeline creation only.
        // We need a way to propagate use to dependent resources
    }
}

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub(crate) enum BindGroupEntry {
    TextureView(TextureHandle), // TODO(andreas) what about non-default views?
    Sampler(SamplerHandle),
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub(crate) struct BindGroupDesc {
    /// Debug label of the bind group. This will show up in graphics debuggers for easy identification.
    pub label: String, // TODO(andreas): Ignore for hashing/comparing?
    pub entries: Vec<BindGroupEntry>,
    pub layout: BindGroupLayoutHandle,
}

pub(crate) struct BindGroupPool {
    pool: ResourcePool<BindGroupHandle, BindGroupDesc, BindGroup>,
}

impl BindGroupPool {
    pub fn new() -> Self {
        BindGroupPool {
            pool: ResourcePool::new(),
        }
    }

    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &BindGroupDesc,
        bind_group_layout: &BindGroupLayoutPool,
        textures: &TexturePool,
        samplers: &SamplerPool,
    ) -> BindGroupHandle {
        self.pool.request(desc, |desc| {
            // TODO(andreas): error handling
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&desc.label),
                entries: &desc
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| wgpu::BindGroupEntry {
                        binding: index as _,
                        resource: match entry {
                            BindGroupEntry::TextureView(handle) => {
                                wgpu::BindingResource::TextureView(
                                    &textures.get(*handle).unwrap().default_view,
                                )
                            }
                            BindGroupEntry::Sampler(handle) => wgpu::BindingResource::Sampler(
                                &samplers.get(*handle).unwrap().sampler,
                            ),
                        },
                    })
                    .collect::<Vec<_>>(),
                layout: &bind_group_layout.get(desc.layout).unwrap().layout,
            });
            BindGroup { bind_group }
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool.frame_maintenance(frame_index);
    }

    pub fn get(&self, handle: BindGroupHandle) -> Result<&BindGroup, PoolError> {
        self.pool.get(handle)
    }
}
