use std::sync::Arc;

use smallvec::SmallVec;

use crate::debug_label::DebugLabel;

use super::{
    bind_group_layout_pool::{GpuBindGroupLayoutHandle, GpuBindGroupLayoutPool},
    buffer_pool::{GpuBuffer, GpuBufferHandle, GpuBufferPool},
    dynamic_resource_pool::{DynamicResource, DynamicResourcePool, SizedResourceDesc},
    sampler_pool::{GpuSamplerHandle, GpuSamplerPool},
    texture_pool::{GpuTexture, GpuTextureHandle, GpuTexturePool},
};

slotmap::new_key_type! { pub struct GpuBindGroupHandle; }

/// A reference counter baked bind group.
///
/// Once instances handles are dropped, the bind group will be marked for reclamation in the following frame.
/// Tracks use of dependent resources as well.
#[derive(Clone)]
pub struct GpuBindGroup {
    resource: Arc<DynamicResource<GpuBindGroupHandle, BindGroupDesc, wgpu::BindGroup>>,
    _owned_buffers: SmallVec<[GpuBuffer; 4]>,
    _owned_textures: SmallVec<[GpuTexture; 4]>,
}

impl std::ops::Deref for GpuBindGroup {
    type Target = wgpu::BindGroup;

    fn deref(&self) -> &Self::Target {
        &self.resource.inner
    }
}

// TODO(andreas): Can we force the user to provide strong handles here without too much effort?
//                Ideally it would be only a reference to a strong handle in order to avoid bumping ref counts all the time.
//                This way we can also remove the dubious get_strong_handle methods from buffer/texture pool and allows us to hide any non-ref counted handles!
//                Seems though this requires us to have duplicate versions of BindGroupDesc/Entry structs

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum BindGroupEntry {
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
pub struct BindGroupDesc {
    /// Debug label of the bind group. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,
    pub entries: SmallVec<[BindGroupEntry; 4]>,
    pub layout: GpuBindGroupLayoutHandle,
}

impl SizedResourceDesc for BindGroupDesc {
    fn resource_size_in_bytes(&self) -> u64 {
        // Size depends on gpu/driver (like with all resources).
        // We could guess something like a pointer per descriptor, but let's not pretend we know!
        0
    }
}

/// Resource pool for bind groups.
///
/// Requirements regarding ownership & resource lifetime:
/// * owned [`wgpu::BindGroup`] should keep buffer/texture alive
///   (user should not need to hold strong buffer/texture handles manually)
/// * [`GpuBindGroupPool`] should *try* to re-use previously created bind groups if they happen to match
/// * musn't prevent buffer/texture re-use on next frame
///   i.e. a internally cached [`GpuBindGroupPool`]s without owner shouldn't keep textures/buffers alive
///
/// We satisfy these by retrieving the strong buffer/texture handles and make them part of the [`GpuBindGroupHandleStrong`].
/// Internally, the [`GpuBindGroupPool`] does *not* hold any strong reference of any resource,
/// i.e. it does not interfere with the buffer/texture pools at all.
/// The question whether a bind groups happen to be re-usable becomes again a simple question of matching
/// bind group descs which itself does not contain any strong references either.
#[derive(Default)]
pub struct GpuBindGroupPool {
    // Use a DynamicResourcePool because it gives out reference counted handles
    // which makes interacting with buffer/textures easier.
    //
    // On the flipside if someone requests the exact same bind group again as before,
    // they'll get a new one which is unnecessary. But this is *very* unlikely to ever happen.
    pool: DynamicResourcePool<GpuBindGroupHandle, BindGroupDesc, wgpu::BindGroup>,
}

impl GpuBindGroupPool {
    /// Returns a ref counted handle to a currently unused bind-group.
    /// Once ownership to the handle is given up, the bind group may be reclaimed in future frames.
    /// The handle also keeps alive any dependent resources.
    pub fn alloc(
        &mut self,
        device: &wgpu::Device,
        desc: &BindGroupDesc,
        bind_group_layouts: &GpuBindGroupLayoutPool,
        textures: &GpuTexturePool,
        buffers: &GpuBufferPool,
        samplers: &GpuSamplerPool,
    ) -> GpuBindGroup {
        // Retrieve strong handles to buffers and textures.
        // This way, an owner of a bind group handle keeps buffers & textures alive!.
        let owned_buffers: SmallVec<[GpuBuffer; 4]> = desc
            .entries
            .iter()
            .filter_map(|e| {
                if let BindGroupEntry::Buffer { handle, .. } = e {
                    Some(
                        buffers
                            .get_from_handle(*handle)
                            .expect("BindGroupDesc had an invalid buffer handle"),
                    )
                } else {
                    None
                }
            })
            .collect();

        let owned_textures: SmallVec<[GpuTexture; 4]> = desc
            .entries
            .iter()
            .filter_map(|e| {
                if let BindGroupEntry::DefaultTextureView(handle) = e {
                    Some(
                        textures
                            .get_from_handle(*handle)
                            .expect("BindGroupDesc had an invalid texture handle"),
                    )
                } else {
                    None
                }
            })
            .collect();

        let resource = self.pool.alloc(desc, |desc| {
            let mut buffer_index = 0;
            let mut texture_index = 0;

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: desc.label.get(),
                entries: &desc
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| wgpu::BindGroupEntry {
                        binding: index as _,
                        resource: match entry {
                            BindGroupEntry::DefaultTextureView(_) => {
                                let res = wgpu::BindingResource::TextureView(
                                    &owned_textures[texture_index].default_view,
                                );
                                texture_index += 1;
                                res
                            }
                            BindGroupEntry::Buffer {
                                handle: _,
                                offset,
                                size,
                            } => {
                                let res = wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: &owned_buffers[buffer_index],
                                    offset: *offset,
                                    size: *size,
                                });
                                buffer_index += 1;
                                res
                            }
                            BindGroupEntry::Sampler(handle) => wgpu::BindingResource::Sampler(
                                samplers
                                    .get_resource(*handle)
                                    .expect("BindGroupDesc had an sampler handle"),
                            ),
                        },
                    })
                    .collect::<Vec<_>>(),
                layout: bind_group_layouts.get_resource(desc.layout).unwrap(),
            })
        });

        GpuBindGroup {
            resource,
            _owned_buffers: owned_buffers,
            _owned_textures: owned_textures,
        }
    }

    pub fn begin_frame(
        &mut self,
        frame_index: u64,
        _textures: &mut GpuTexturePool,
        _buffers: &mut GpuBufferPool,
        _samplers: &mut GpuSamplerPool,
    ) {
        self.pool.begin_frame(frame_index, |_res| {});
        // TODO(andreas): Update usage counter on dependent resources.
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }
}
