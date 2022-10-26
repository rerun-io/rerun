use std::sync::atomic::AtomicU64;

use crate::debug_label::DebugLabel;

use super::{
    bind_group_layout_pool::{BindGroupLayoutHandle, BindGroupLayoutPool},
    buffer_pool::{BufferHandle, BufferPool},
    resource::*,
    sampler_pool::{SamplerHandle, SamplerPool},
    static_resource_pool::*,
    texture_pool::{TextureHandle, TexturePool},
};

slotmap::new_key_type! { pub(crate) struct BindGroupHandle; }

pub(crate) struct BindGroup {
    last_frame_used: AtomicU64,
    pub(crate) bind_group: wgpu::BindGroup,
}

// [`BindGroup`] is relatively lightweight, but since buffers and textures are recreated a lot, we might pile them up, so let's keep track!
impl UsageTrackedResource for BindGroup {
    fn last_frame_used(&self) -> &AtomicU64 {
        &self.last_frame_used
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) enum BindGroupEntry {
    TextureView(TextureHandle), // TODO(andreas) what about non-default views?
    Buffer {
        handle: BufferHandle,

        /// Base offset of the buffer. For bindings with `dynamic == true`, this offset
        /// will be added to the dynamic offset provided in [`wgpu::RenderPass::set_bind_group`].
        ///
        /// The offset has to be aligned to [`wgpu::Limits::min_uniform_buffer_offset_alignment`]
        /// or [`wgpu::Limits::min_storage_buffer_offset_alignment`] appropriately.
        offset: wgpu::BufferAddress,

        /// Size of the binding, or `None` for using the rest of the buffer.
        size: Option<wgpu::BufferSize>,
    },
    Sampler(SamplerHandle),
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) struct BindGroupDesc {
    /// Debug label of the bind group. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,
    pub entries: Vec<BindGroupEntry>,
    pub layout: BindGroupLayoutHandle,
}

// TODO: proper doc of what we're doing

// Different expectations regarding ownership:
// * alloc buffer/texture, pass it to bind group, throw handle to buffer/texture away, hold on to bind group indefinitely
//      => BindGroup should keep buffer/texture alive!
// * alloc buffer/texture, pass it to bind group, throw both away, do the same next frame
//      => BindGroupPool should *try* to re-use previously created bind groups!
//      => musn't prevent buffer/texture re-use on next frame
#[derive(Default)]
pub(crate) struct BindGroupPool {
    pool: StaticResourcePool<BindGroupHandle, BindGroupDesc, BindGroup>,
}

impl BindGroupPool {
    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &BindGroupDesc,
        bind_group_layout: &BindGroupLayoutPool,
        textures: &TexturePool,
        buffers: &BufferPool,
        samplers: &SamplerPool,
    ) -> BindGroupHandle {
        self.pool.get_or_create(desc, |desc| {
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
                            BindGroupEntry::Buffer {
                                handle,
                                offset,
                                size,
                            } => wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &buffers.get_resource(handle).unwrap().buffer,
                                offset: *offset,
                                size: *size,
                            }),
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

    pub fn frame_maintenance(
        &mut self,
        frame_index: u64,
        textures: &mut TexturePool,
        buffers: &mut BufferPool,
        samplers: &mut SamplerPool,
    ) {
        self.pool.discard_unused_resources(frame_index);

        // Of what's left, update dependent resources.
        for desc in self.pool.resource_descs() {
            for entry in &desc.entries {
                match entry {
                    BindGroupEntry::TextureView(handle) => {
                        textures.register_resource_usage(*handle);
                    }
                    BindGroupEntry::Buffer { handle, .. } => {
                        buffers.register_resource_usage(handle);
                    }
                    BindGroupEntry::Sampler(handle) => {
                        samplers.register_resource_usage(*handle);
                    }
                }
            }
        }
    }

    pub fn get(&self, handle: BindGroupHandle) -> Result<&BindGroup, PoolError> {
        self.pool.get_resource(handle)
    }
}
