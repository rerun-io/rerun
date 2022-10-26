use std::{hash::Hash, sync::atomic::AtomicU64};

use crate::debug_label::DebugLabel;

use super::{dynamic_resource_pool::DynamicResourcePool, resource::*};

slotmap::new_key_type! { pub struct BufferHandle; }

/// A reference counter baked bind group handle.
/// Once all strong handles are dropped, the bind group will be marked for reclamation in the following frame.
pub type BufferHandleStrong = std::sync::Arc<BufferHandle>;

pub struct Buffer {
    last_frame_used: AtomicU64,
    pub buffer: wgpu::Buffer,
}

impl UsageTrackedResource for Buffer {
    fn last_frame_used(&self) -> &AtomicU64 {
        &self.last_frame_used
    }
}

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

#[derive(Default)]
pub struct BufferPool {
    pool: DynamicResourcePool<BufferHandle, BufferDesc, Buffer>,
}

impl BufferPool {
    pub fn alloc(
        &mut self,
        device: &wgpu::Device,
        desc: &BufferDesc,
    ) -> anyhow::Result<BufferHandleStrong> {
        self.pool.alloc(desc, |desc| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: desc.label.get(),
                size: desc.size,
                usage: desc.usage,
                mapped_at_creation: false,
            });
            Ok(Buffer {
                last_frame_used: AtomicU64::new(0),
                buffer,
            })
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool.frame_maintenance(frame_index);
    }

    /// Takes strong buffer handle to ensure the user is still holding on to the buffer.
    pub fn get_resource(&self, handle: &BufferHandleStrong) -> Result<&Buffer, PoolError> {
        self.pool.get_resource(**handle)
    }

    /// Internal method to retrieve a resource with a weak handle (used by [`BindGroupPool`])
    pub(super) fn get_resource_weak(&self, handle: BufferHandle) -> Result<&Buffer, PoolError> {
        self.pool.get_resource(handle)
    }

    /// Internal method to retrieve a strong handle from a weak handle (used by [`BindGroupPool`])
    pub(super) fn get_strong_handle(&self, handle: BufferHandle) -> &BufferHandleStrong {
        self.pool.get_strong_handle(handle)
    }
}
