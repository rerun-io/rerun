use std::sync::atomic::AtomicU64;

use crate::debug_label::DebugLabel;

use super::{
    bind_group_layout_pool::{BindGroupLayoutHandle, BindGroupLayoutPool},
    resource_pool::*,
    sampler_pool::{SamplerHandle, SamplerPool},
    texture_pool::{TextureHandle, TexturePool},
};

slotmap::new_key_type! { pub(crate) struct BindGroupHandle; }

pub(crate) struct BindGroup {
    last_frame_used: AtomicU64,
    pub(crate) bind_group: wgpu::BindGroup,
}

// BindGroup is relatively lightweight, but since buffers and textures are recreated a lot, we might pile them up, so let's keep track!
impl UsageTrackedResource for BindGroup {
    fn last_frame_used(&self) -> &AtomicU64 {
        &self.last_frame_used
    }
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) enum BindGroupEntry {
    TextureView(TextureHandle), // TODO(andreas) what about non-default views?
    Sampler(SamplerHandle),
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) struct BindGroupDesc {
    /// Debug label of the bind group. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,
    pub entries: Vec<BindGroupEntry>,
    pub layout: BindGroupLayoutHandle,
}

#[derive(Default)]
pub(crate) struct BindGroupPool {
    pool: ResourcePool<BindGroupHandle, BindGroupDesc, BindGroup>,
}

impl BindGroupPool {
    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &BindGroupDesc,
        bind_group_layout: &BindGroupLayoutPool,
        textures: &TexturePool,
        samplers: &SamplerPool,
    ) -> BindGroupHandle {
        self.pool.get_handle(desc, |desc| {
            // TODO(andreas): error handling
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: desc.label.get(),
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
            BindGroup {
                bind_group,
                last_frame_used: AtomicU64::new(0),
            }
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64, textures: &mut TexturePool) {
        self.pool.discard_unused_resources(frame_index);

        // Of what's left, update dependent resources.
        for desc in self.pool.resource_descs() {
            for entry in &desc.entries {
                match entry {
                    BindGroupEntry::TextureView(handle) => {
                        textures.register_resource_usage(*handle);
                    }
                    BindGroupEntry::Sampler(_) => {} // Samplers don't track frame index
                }
            }
        }
    }

    pub fn get(&self, handle: BindGroupHandle) -> Result<&BindGroup, PoolError> {
        self.pool.get_resource(handle)
    }
}
