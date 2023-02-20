use crate::debug_label::DebugLabel;

use super::{resource::PoolError, static_resource_pool::StaticResourcePool};

slotmap::new_key_type! { pub struct GpuBindGroupLayoutHandle; }

#[derive(Clone, Hash, PartialEq, Eq, Default)]
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
        &mut self,
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

    pub fn get_resource(
        &self,
        handle: GpuBindGroupLayoutHandle,
    ) -> Result<&wgpu::BindGroupLayout, PoolError> {
        self.pool.get_resource(handle)
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        self.pool.current_frame_index = frame_index;
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }
}
