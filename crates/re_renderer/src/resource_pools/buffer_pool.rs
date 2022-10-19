use std::{hash::Hash, sync::atomic::AtomicU64};

use crate::debug_label::DebugLabel;

use super::resource_pool::*;

slotmap::new_key_type! { pub(crate) struct BufferHandle; }

pub(crate) struct Buffer {
    last_frame_used: AtomicU64,
    pub(crate) buffer: wgpu::Buffer,
}

impl UsageTrackedResource for Buffer {
    fn last_frame_used(&self) -> &AtomicU64 {
        &self.last_frame_used
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub(crate) struct BufferDesc {
    /// Debug label of a buffer. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    /// Size of a buffer.
    pub size: wgpu::BufferAddress,
    /// Usages of a buffer. If the buffer is used in any way that isn't specified here, the operation
    /// will panic.
    pub usage: wgpu::BufferUsages,

    /// Content id used to distinguish otherwise identical buffer descriptions.
    pub content_id: u64,
    // TODO(andreas) support buffers that are mapped on creation.
    // This is *not* the same as mapping the buffer right after creation as it allows mapping even without
    // [`wgpu::BufferUsages::MAP_WRITE`] flag set!
    // On the other hand this isn't an amazing fit with our design overall as we'd like to pretend
    // we never know when a buffer is created the first time.
    //pub mapped_at_creation: bool,
}

#[derive(Default)]
pub(crate) struct BufferPool {
    pool: ResourcePool<BufferHandle, BufferDesc, Buffer>,
}

impl BufferPool {
    pub fn request(&mut self, device: &wgpu::Device, desc: &BufferDesc) -> BufferHandle {
        self.pool.get_handle(desc, |desc| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: desc.label.get(),
                size: desc.size,
                usage: desc.usage,
                mapped_at_creation: false,
            });
            Buffer {
                last_frame_used: AtomicU64::new(0),
                buffer,
            }
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool.discard_unused_resources(frame_index);
    }

    pub fn get(&self, handle: BufferHandle) -> Result<&Buffer, PoolError> {
        self.pool.get_resource(handle)
    }

    pub(super) fn register_resource_usage(&mut self, handle: BufferHandle) {
        let _ = self.get(handle);
    }
}
