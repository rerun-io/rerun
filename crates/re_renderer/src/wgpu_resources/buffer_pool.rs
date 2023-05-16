use std::hash::Hash;

use crate::debug_label::DebugLabel;

use super::{
    dynamic_resource_pool::{DynamicResource, DynamicResourcePool, DynamicResourcesDesc},
    resource::PoolError,
};

slotmap::new_key_type! { pub struct GpuBufferHandle; }

/// A reference-counter baked buffer.
/// Once all instances are dropped, the buffer will be marked for reclamation in the following frame.
pub type GpuBuffer = std::sync::Arc<DynamicResource<GpuBufferHandle, BufferDesc, wgpu::Buffer>>;

/// Buffer creation descriptor.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct BufferDesc {
    /// Debug label of a buffer. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    /// Size of a buffer.
    pub size: wgpu::BufferAddress,

    /// Usages of a buffer. If the buffer is used in any way that isn't specified here, the operation
    /// will panic.
    pub usage: wgpu::BufferUsages,

    /// Allows a buffer to be mapped immediately after they are made. It does not have to be [`wgpu::BufferUsages::MAP_READ`] or
    /// [`wgpu::BufferUsages::MAP_WRITE`], all buffers are allowed to be mapped at creation.
    ///
    /// *WARNING*: If this is `true`, the pool won't be able to reclaim the buffer later!
    /// Furthermore, [`size`](#structfield.size) must be a multiple of [`wgpu::COPY_BUFFER_ALIGNMENT`].
    pub mapped_at_creation: bool,
}

impl DynamicResourcesDesc for BufferDesc {
    fn resource_size_in_bytes(&self) -> u64 {
        self.size
    }

    fn allow_reuse(&self) -> bool {
        // We can't re-use buffers that were mapped at creation since we don't know if the user
        // unmapped the buffer.
        // We could try to figure it out, but mapped-at-creation buffers should only be used by one of the dedicated allocators anyways!
        !self.mapped_at_creation
    }
}

#[derive(Default)]
pub struct GpuBufferPool {
    pool: DynamicResourcePool<GpuBufferHandle, BufferDesc, wgpu::Buffer>,
}

impl GpuBufferPool {
    /// Returns a reference-counted gpu buffer that is currently unused.
    /// Once ownership is given up, the buffer may be reclaimed in future frames
    /// unless `BufferDesc::mapped_at_creation` was true.
    ///
    /// For more efficient allocation (faster, less fragmentation) you should sub-allocate buffers whenever possible
    /// either manually or using a higher level allocator.
    pub fn alloc(&self, device: &wgpu::Device, desc: &BufferDesc) -> GpuBuffer {
        crate::profile_function!();
        self.pool.alloc(desc, |desc| {
            crate::profile_scope!("create_buffer");
            device.create_buffer(&wgpu::BufferDescriptor {
                label: desc.label.get(),
                size: desc.size,
                usage: desc.usage,
                mapped_at_creation: desc.mapped_at_creation,
            })
        })
    }

    /// Called by `RenderContext` every frame. Updates statistics and may free unused buffers.
    pub fn begin_frame(&mut self, frame_index: u64) {
        self.pool.begin_frame(frame_index, |res| res.destroy());
    }

    /// Method to retrieve a resource from a weak handle (used by [`super::GpuBindGroupPool`])
    pub fn get_from_handle(&self, handle: GpuBufferHandle) -> Result<GpuBuffer, PoolError> {
        self.pool.get_from_handle(handle)
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }

    pub fn total_gpu_size_in_bytes(&self) -> u64 {
        self.pool.total_resource_size_in_bytes()
    }
}
