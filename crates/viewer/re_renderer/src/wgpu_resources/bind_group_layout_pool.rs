use super::static_resource_pool::{StaticResourcePool, StaticResourcePoolReadLockAccessor};
use crate::debug_label::DebugLabel;

slotmap::new_key_type! { pub struct GpuBindGroupLayoutHandle; }

#[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
pub struct BindGroupLayoutDesc {
    /// Debug label of the bind group layout. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,
    pub entries: Vec<wgpu::BindGroupLayoutEntry>,
}

#[derive(Default)]
pub struct GpuBindGroupLayoutPool {
    pool: StaticResourcePool<GpuBindGroupLayoutHandle, BindGroupLayoutDesc, wgpu::BindGroupLayout>,
}

impl GpuBindGroupLayoutPool {
    pub fn get_or_create(
        &self,
        device: &wgpu::Device,
        desc: &BindGroupLayoutDesc,
    ) -> GpuBindGroupLayoutHandle {
        self.pool.get_or_create(desc, |desc| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: desc.label.get(),
                entries: &desc.entries,
            })
        })
    }

    /// Locks the resource pool for resolving handles.
    ///
    /// While it is locked, no new resources can be added.
    pub fn resources(
        &self,
    ) -> StaticResourcePoolReadLockAccessor<'_, GpuBindGroupLayoutHandle, wgpu::BindGroupLayout>
    {
        self.pool.resources()
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        self.pool.current_frame_index = frame_index;
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }

    /// Register a pre-existing `wgpu::BindGroupLayout` and return a pool handle.
    ///
    /// Used when the layout was created externally (e.g. by a custom shader bind group builder).
    pub fn register_existing(&self, layout: wgpu::BindGroupLayout) -> GpuBindGroupLayoutHandle {
        self.pool.insert_resource(layout)
    }
}
