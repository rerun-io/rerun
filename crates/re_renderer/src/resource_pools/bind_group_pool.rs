use std::sync::{atomic::AtomicU64, Arc};

use smallvec::SmallVec;

use crate::debug_label::DebugLabel;

use super::{
    bind_group_layout_pool::{BindGroupLayoutHandle, BindGroupLayoutPool},
    buffer_pool::{BufferHandle, BufferPool, StrongBufferHandle},
    dynamic_resource_pool::DynamicResourcePool,
    resource::*,
    sampler_pool::{SamplerHandle, SamplerPool},
    texture_pool::{TextureHandle, TextureHandleStrong, TexturePool},
};

slotmap::new_key_type! { pub struct BindGroupHandle; }

#[derive(Clone)]
pub struct StrongBindGroupHandle {
    handle: Arc<BindGroupHandle>,
    _owned_buffers: SmallVec<[StrongBufferHandle; 4]>,
    _owned_textures: SmallVec<[TextureHandleStrong; 4]>,
}

impl std::ops::Deref for StrongBindGroupHandle {
    type Target = BindGroupHandle;

    fn deref(&self) -> &Self::Target {
        &*self.handle
    }
}

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

/// Different expectations regarding ownership:
/// * alloc buffer/texture, pass it to bind group, throw handle to buffer/texture away, hold on to bind group indefinitely
///      => owned [`BindGroup`] should keep buffer/texture alive!
/// * alloc buffer/texture, pass it to bind group, throw both away, do the same next frame
///      => BindGroupPool should *try* to re-use previously created bind groups!
///      => musn't prevent buffer/texture re-use on next frame, i.e. BindGroupPools without owner shouldn't keep anyone alive
///
/// We solve all this by retrieving the strong buffer/texture handles and make them part of the strong BindGroupHandle.
/// Internally, the BindGroupPool does *not* hold any strong reference to any resource,
/// i.e. it does not interfere with the buffer/texture pools at all.
/// The question whether a bind groups happen to be re-usable becomes again a simple question of matching
/// bind group descs which itself does not contain any strong references either.
#[derive(Default)]
pub(crate) struct BindGroupPool {
    pool: DynamicResourcePool<BindGroupHandle, BindGroupDesc, BindGroup>,
}

impl BindGroupPool {
    pub fn alloc(
        &mut self,
        device: &wgpu::Device,
        desc: &BindGroupDesc,
        bind_group_layout: &BindGroupLayoutPool,
        textures: &TexturePool,
        buffers: &BufferPool,
        samplers: &SamplerPool,
    ) -> anyhow::Result<StrongBindGroupHandle> {
        let handle = self.pool.alloc(&desc, |desc| {
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
                                    &textures.get_resource_weak(*handle).unwrap().default_view,
                                )
                            }
                            BindGroupEntry::Buffer {
                                handle,
                                offset,
                                size,
                            } => wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &buffers.get_resource_weak(*handle).unwrap().buffer,
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
            Ok(BindGroup {
                bind_group,
                last_frame_used: AtomicU64::new(0),
            })
        })?;

        // Retrieve strong handles to buffers and textures.
        // This way, an owner of a bind group handle keeps buffers & textures alive!.
        let owned_buffers = desc
            .entries
            .iter()
            .filter_map(|e| {
                if let BindGroupEntry::Buffer {
                    handle,
                    offset: _,
                    size: _,
                } = e
                {
                    Some(buffers.get_strong_handle(*handle).clone())
                } else {
                    None
                }
            })
            .collect();

        let owned_textures = desc
            .entries
            .iter()
            .filter_map(|e| {
                if let BindGroupEntry::TextureView(handle) = e {
                    Some(textures.get_strong_handle(*handle).clone())
                } else {
                    None
                }
            })
            .collect();

        Ok(StrongBindGroupHandle {
            handle,
            _owned_buffers: owned_buffers,
            _owned_textures: owned_textures,
        })
    }

    pub fn frame_maintenance(
        &mut self,
        frame_index: u64,
        _textures: &mut TexturePool,
        _buffers: &mut BufferPool,
        _samplers: &mut SamplerPool,
    ) {
        self.pool.frame_maintenance(frame_index);
        // TODO(andreas): Update usage counter on dependent resources.
    }

    /// Takes a strong handle to ensure the user is still holding on to the bind group (and thus dependent resources).
    pub fn get_resource(&self, handle: &StrongBindGroupHandle) -> Result<&BindGroup, PoolError> {
        self.pool.get_resource(*handle.handle)
    }
}
