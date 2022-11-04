use std::sync::{atomic::AtomicU64, Arc};

use smallvec::SmallVec;

use crate::debug_label::DebugLabel;

use super::{
    bind_group_layout_pool::{GpuBindGroupLayoutHandle, GpuBindGroupLayoutPool},
    buffer_pool::{GpuBufferHandle, GpuBufferHandleStrong, GpuBufferPool},
    dynamic_resource_pool::DynamicResourcePool,
    resource::*,
    sampler_pool::{GpuSamplerHandle, GpuSamplerPool},
    texture_pool::{GpuTextureHandle, GpuTextureHandleStrong, TexturePool},
};

slotmap::new_key_type! { pub struct GpuBindGroupHandle; }

/// A reference counter baked bind group handle.
///
/// Once all strong handles are dropped, the bind group will be marked for reclamation in the following frame.
/// Tracks use of dependent resources as well.
#[derive(Clone)]
pub struct GpuBindGroupHandleStrong {
    handle: Arc<GpuBindGroupHandle>,
    _owned_buffers: SmallVec<[GpuBufferHandleStrong; 4]>,
    _owned_textures: SmallVec<[GpuTextureHandleStrong; 4]>,
}

impl std::ops::Deref for GpuBindGroupHandleStrong {
    type Target = GpuBindGroupHandle;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

pub(crate) struct GpuBindGroup {
    last_frame_used: AtomicU64,
    pub(crate) bind_group: wgpu::BindGroup,
}

// [`BindGroup`] is relatively lightweight, but since buffers and textures are recreated a lot, we might pile them up, so let's keep track!
impl UsageTrackedResource for GpuBindGroup {
    fn last_frame_used(&self) -> &AtomicU64 {
        &self.last_frame_used
    }
}

// TODO(andreas): Can we force the user to provide strong handles here without too much effort?
//                Ideally it would be only a reference to a strong handle in order to avoid bumping ref counts all the time.
//                This way we can also remove the dubious get_strong_handle methods from buffer/texture pool and allows us to hide any non-ref counted handles!
//                Seems though this requires us to have duplicate versions of BindGroupDesc/Entry structs

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) enum BindGroupEntry {
    DefaultTextureView(GpuTextureHandle), // TODO(andreas) what about non-default views?
    Buffer {
        handle: GpuBufferHandle,

        /// Base offset of the buffer. For bindings with `dynamic == true`, this offset
        /// will be added to the dynamic offset provided in [`wgpu::RenderPass::set_bind_group`].
        ///
        /// The offset has to be aligned to [`wgpu::Limits::min_uniform_buffer_offset_alignment`]
        /// or [`wgpu::Limits::min_storage_buffer_offset_alignment`] appropriately.
        offset: wgpu::BufferAddress,

        /// Size of the binding, or `None` for using the rest of the buffer.
        size: Option<wgpu::BufferSize>,
    },
    Sampler(GpuSamplerHandle),
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) struct BindGroupDesc {
    /// Debug label of the bind group. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,
    pub entries: SmallVec<[BindGroupEntry; 4]>,
    pub layout: GpuBindGroupLayoutHandle,
}

/// Resource pool for bind groups.
///
/// Requirements regarding ownership & resource lifetime:
/// * owned [`BindGroup`] should keep buffer/texture alive
///   (user should not need to hold strong buffer/texture handles manually)
/// * [`BindGroupPool`] should *try* to re-use previously created bind groups if they happen to match
/// * musn't prevent buffer/texture re-use on next frame
///   i.e. a internally cached [`BindGroupPool`]s without owner shouldn't keep textures/buffers alive
///
/// We satisfy these by retrieving the strong buffer/texture handles and make them part of the [`BindGroupHandleStrong`].
/// Internally, the [`BindGroupPool`] does *not* hold any strong reference of any resource,
/// i.e. it does not interfere with the buffer/texture pools at all.
/// The question whether a bind groups happen to be re-usable becomes again a simple question of matching
/// bind group descs which itself does not contain any strong references either.
#[derive(Default)]
pub(crate) struct GpuBindGroupPool {
    // Use a DynamicResourcePool because it gives out reference counted handles
    // which makes interacting with buffer/textures easier.
    //
    // On the flipside if someone requests the exact same bind group again as before,
    // they'll get a new one which is unnecessary. But this is *very* unlikely to ever happen.
    pool: DynamicResourcePool<GpuBindGroupHandle, BindGroupDesc, GpuBindGroup>,
}

impl GpuBindGroupPool {
    /// Returns a ref counted handle to a currently unused bind-group.
    /// Once ownership to the handle is given up, the bind group may be reclaimed in future frames.
    /// The handle also keeps alive any dependent resources.
    pub fn alloc(
        &mut self,
        device: &wgpu::Device,
        desc: &BindGroupDesc,
        bind_group_layout: &GpuBindGroupLayoutPool,
        textures: &TexturePool,
        buffers: &GpuBufferPool,
        samplers: &GpuSamplerPool,
    ) -> GpuBindGroupHandleStrong {
        let handle = self.pool.alloc(desc, |desc| {
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
                            BindGroupEntry::DefaultTextureView(handle) => {
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
                                &samplers.get_resource(*handle).unwrap().sampler,
                            ),
                        },
                    })
                    .collect::<Vec<_>>(),
                layout: &bind_group_layout.get_resource(desc.layout).unwrap().layout,
            });
            GpuBindGroup {
                bind_group,
                last_frame_used: AtomicU64::new(0),
            }
        });

        // Retrieve strong handles to buffers and textures.
        // This way, an owner of a bind group handle keeps buffers & textures alive!.
        let owned_buffers = desc
            .entries
            .iter()
            .filter_map(|e| {
                if let BindGroupEntry::Buffer { handle, .. } = e {
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
                if let BindGroupEntry::DefaultTextureView(handle) = e {
                    Some(textures.get_strong_handle(*handle).clone())
                } else {
                    None
                }
            })
            .collect();

        GpuBindGroupHandleStrong {
            handle,
            _owned_buffers: owned_buffers,
            _owned_textures: owned_textures,
        }
    }

    pub fn frame_maintenance(
        &mut self,
        frame_index: u64,
        _textures: &mut TexturePool,
        _buffers: &mut GpuBufferPool,
        _samplers: &mut GpuSamplerPool,
    ) {
        self.pool.frame_maintenance(frame_index);
        // TODO(andreas): Update usage counter on dependent resources.
    }

    /// Takes a strong handle to ensure the user is still holding on to the bind group (and thus dependent resources).
    pub fn get_resource(
        &self,
        handle: &GpuBindGroupHandleStrong,
    ) -> Result<&GpuBindGroup, PoolError> {
        self.pool.get_resource(*handle.handle)
    }
}
