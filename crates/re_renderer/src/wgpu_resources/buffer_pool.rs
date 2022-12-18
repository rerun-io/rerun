use std::{hash::Hash, sync::Arc};

use crate::debug_label::DebugLabel;

use super::{
    dynamic_resource_pool::{DynamicResourcePool, SizedResourceDesc},
    resource::PoolError,
};

slotmap::new_key_type! { pub struct GpuBufferHandle; }

/// A reference counter baked bind group handle.
/// Once all strong handles are dropped, the bind group will be marked for reclamation in the following frame.
pub type GpuBufferHandleStrong = Arc<GpuBufferHandle>;

/// Buffer creation descriptor.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct BufferDesc {
    /// Debug label of a buffer. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    /// Size of a buffer.
    pub size: wgpu::BufferAddress,

    /// Usages of a buffer. If the buffer is used in any way that isn't specified here, the operation
    /// will panic.
    ///
    /// For wgpu::BufferUsages::MAP_WRITE, use `alloc_staging_write_buffer`
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

    /// List of all write staging buffers returned.
    write_staging_buffers: Vec<StagingWriteBuffer>,
    total_write_staging_buffer_size: u64,
}

#[derive(Clone)]
pub struct StagingWriteBuffer {
    wgpu_buffer: Arc<wgpu::Buffer>,
}

impl SizedResourceDesc for StagingWriteBuffer {
    #[inline]
    fn resource_size_in_bytes(&self) -> u64 {
        self.wgpu_buffer.size()
    }
}

impl std::ops::Deref for StagingWriteBuffer {
    type Target = wgpu::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.wgpu_buffer
    }
}

impl GpuBufferPool {
    /// Allocates a cpu writable & gpu readable buffer that is mapped immediately and
    /// has no direct lifetime dependency on this pool.
    ///
    /// The later is important to be able to pass around memory slices that are derived from this buffer,
    /// without the explicit knowledge of it being data from re_renderer.
    /// We could handle this by injecting the wgpu::Buffer into the strong buffer handle itself,
    /// but treating this as a special kind of resources is advantageous since any resource with MAP_WRITE
    /// can only be used with COPY_SRC!
    ///
    /// TODO(andreas): Consider a separate resource pool for read & write staging buffers.
    ///
    /// ⚠️ Forces creation of a new buffer that cannot be reclaimed later ⚠️
    /// This is because all other mapping operations are asynchronous. We could still allow
    /// re-use by implementing a mechanism similar to the re-use strategy [`crate::StagingBelt`] employs,
    /// but as of writing this is the only user needing in need of creation mapped buffer in the first place.
    pub fn alloc_staging_write_buffer(
        &mut self,
        device: &wgpu::Device,
        label: DebugLabel,
        size: wgpu::BufferAddress,
    ) -> StagingWriteBuffer {
        let wgpu_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: label.get(),
            size,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: true,
        });

        let buffer = StagingWriteBuffer {
            wgpu_buffer: Arc::new(wgpu_buffer),
        };

        self.total_write_staging_buffer_size += buffer.resource_size_in_bytes();
        self.write_staging_buffers.push(buffer.clone());

        buffer
    }

    /// Returns a ref counted handle to a currently unused buffer.
    /// Once ownership to the handle is given up, the buffer may be reclaimed in future frames,
    /// unless re-use was bypassed by [`BufferDesc::bypass_reuse_and_map_on_creation`]
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
    pub fn begin_frame(&mut self, frame_index: u64) {
        self.pool.begin_frame(frame_index, |res| res.destroy());

        self.write_staging_buffers.retain(|buffer| {
            if Arc::strong_count(&buffer.wgpu_buffer) == 1 {
                self.total_write_staging_buffer_size -= buffer.resource_size_in_bytes();
                buffer.wgpu_buffer.destroy();
                false
            } else {
                true
            }
        });
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
        // Technically staging buffers aren't gpu memory, but we count them towards this
        // anyways as they typically (!) don't show up in application side cpu tracked memory.
        self.pool.total_resource_size_in_bytes() + self.total_write_staging_buffer_size
    }
}
