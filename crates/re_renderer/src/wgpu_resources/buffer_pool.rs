use std::hash::Hash;

use crate::debug_label::DebugLabel;

use super::{
    dynamic_resource_pool::{DynamicResourcePool, SizedResourceDesc},
    resource::PoolError,
};

slotmap::new_key_type! { pub struct GpuBufferHandle; }

/// A reference counter baked bind group handle.
/// Once all strong handles are dropped, the bind group will be marked for reclamation in the following frame.
pub type GpuBufferHandleStrong = std::sync::Arc<GpuBufferHandle>;

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct BufferDesc {
    /// Debug label of a buffer. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    /// Size of a buffer.
    pub size: wgpu::BufferAddress,

    /// Usages of a buffer. If the buffer is used in any way that isn't specified here, the operation
    /// will panic.
    pub usage: wgpu::BufferUsages,
}

impl SizedResourceDesc for BufferDesc {
    fn resource_size_in_bytes(&self) -> u64 {
        self.size
    }
}

#[derive(Default)]
pub struct GpuBufferPool {
    pool: DynamicResourcePool<GpuBufferHandle, BufferDesc, wgpu::Buffer>,
}

impl GpuBufferPool {
    /// Returns a ref counted handle to a currently unused buffer.
    /// Once ownership to the handle is given up, the buffer may be reclaimed in future frames.
    ///
    /// For more efficient allocation (faster, less fragmentation) you should sub-allocate buffers whenever possible
    /// either manually or using a higher level allocator.
    pub fn alloc(&mut self, device: &wgpu::Device, desc: &BufferDesc) -> GpuBufferHandleStrong {
        self.pool.alloc(desc, |desc| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: desc.label.get(),
                size: desc.size,
                usage: desc.usage,
                mapped_at_creation: false,
            })
        })
    }

    /// Called by `RenderContext` every frame. Updates statistics and may free unused buffers.
    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool
            .frame_maintenance(frame_index, |res| res.destroy());
    }

    /// Takes strong buffer handle to ensure the user is still holding on to the buffer.
    pub fn get_resource(&self, handle: &GpuBufferHandleStrong) -> Result<&wgpu::Buffer, PoolError> {
        self.pool.get_resource(**handle)
    }

    /// Internal method to retrieve a resource with a weak handle (used by [`super::GpuBindGroupPool`])
    pub(super) fn get_resource_weak(
        &self,
        handle: GpuBufferHandle,
    ) -> Result<&wgpu::Buffer, PoolError> {
        self.pool.get_resource(handle)
    }

    /// Internal method to retrieve a strong handle from a weak handle (used by [`super::GpuBindGroupPool`])
    /// without inrementing the ref-count (note the returned reference!).
    pub(super) fn get_strong_handle(&self, handle: GpuBufferHandle) -> &GpuBufferHandleStrong {
        self.pool.get_strong_handle(handle)
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }

    pub fn total_gpu_size_in_bytes(&self) -> u64 {
        self.pool.total_resource_size_in_bytes()
    }
}
