use crate::debug_label::DebugLabel;

use super::{resource::*, static_resource_pool::*};

slotmap::new_key_type! { pub struct GpuBindGroupLayoutHandle; }

pub struct GpuBindGroupLayout {
    pub layout: wgpu::BindGroupLayout,
}

impl GpuResource for GpuBindGroupLayout {}

#[derive(Clone, Hash, PartialEq, Eq, Default)]
pub struct BindGroupLayoutDesc {
    /// Debug label of the bind group layout. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,
    pub entries: Vec<wgpu::BindGroupLayoutEntry>,
}

#[derive(Default)]
pub struct GpuBindGroupLayoutPool {
    pool: StaticResourcePool<GpuBindGroupLayoutHandle, BindGroupLayoutDesc, GpuBindGroupLayout>,
}

impl GpuBindGroupLayoutPool {
    pub fn get_or_create(
        &mut self,
        device: &wgpu::Device,
        desc: &BindGroupLayoutDesc,
    ) -> GpuBindGroupLayoutHandle {
        self.pool.get_or_create(desc, |desc| {
            // TODO(andreas): error handling
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: desc.label.get(),
                entries: &desc.entries,
            });
            GpuBindGroupLayout { layout }
        })
    }

    pub fn get_resource(
        &self,
        handle: GpuBindGroupLayoutHandle,
    ) -> Result<&GpuBindGroupLayout, PoolError> {
        self.pool.get_resource(handle)
    }
}
