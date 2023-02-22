use std::hash::Hash;

use crate::debug_label::DebugLabel;

use super::{
    dynamic_resource_pool::{DynamicResource, DynamicResourcePool, SizedResourceDesc},
    resource::PoolError,
};

slotmap::new_key_type! { pub struct GpuBufferHandle; }

/// A reference-counter baked buffer.
/// Once all instances are dropped, the buffer will be marked for reclamation in the following frame.
pub type GpuBuffer = std::sync::Arc<DynamicResource<GpuBufferHandle, BufferDesc, wgpu::Buffer>>;

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
    /// Returns a reference-counted gpu buffer that is currently unused.
    /// Once ownership is given up, the buffer may be reclaimed in future frames.
    ///
    /// For more efficient allocation (faster, less fragmentation) you should sub-allocate buffers whenever possible
    /// either manually or using a higher level allocator.
    pub fn alloc(&mut self, device: &wgpu::Device, desc: &BufferDesc) -> GpuBuffer {
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
    pub fn begin_frame(&mut self, frame_index: u64) {
        self.pool.begin_frame(frame_index, |res| res.destroy());
    }

    /// Internal method to retrieve a resource from a weak handle (used by [`super::GpuBindGroupPool`])
    pub(super) fn get_from_handle(&self, handle: GpuBufferHandle) -> Result<GpuBuffer, PoolError> {
        self.pool.get_from_handle(handle)
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }

    pub fn total_gpu_size_in_bytes(&self) -> u64 {
        self.pool.total_resource_size_in_bytes()
    }
}
