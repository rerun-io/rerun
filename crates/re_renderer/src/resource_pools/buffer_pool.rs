use std::{hash::Hash, sync::atomic::AtomicU64};

use crate::debug_label::DebugLabel;

use super::resource_pool::*;

slotmap::new_key_type! { pub struct BufferHandleInner; }
pub type BufferHandle = std::sync::Arc<BufferHandleInner>;

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
    pool: DynamicResourcePool<BufferHandleInner, BufferDesc, Buffer>,
}

impl BufferPool {
    pub fn alloc(
        &mut self,
        device: &wgpu::Device,
        desc: &BufferDesc,
    ) -> anyhow::Result<BufferHandle> {
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

    pub fn get_resource(&self, handle: &BufferHandle) -> Result<&Buffer, PoolError> {
        self.pool.get_resource(handle)
    }

    pub(super) fn register_resource_usage(&mut self, handle: &BufferHandle) {
        let _ = self.get_resource(handle);
    }
}
