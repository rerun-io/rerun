use crate::debug_label::DebugLabel;

use super::{resource::*, static_resource_pool::*};

slotmap::new_key_type! { pub(crate) struct BindGroupLayoutHandle; }

pub(crate) struct BindGroupLayout {
    pub(crate) layout: wgpu::BindGroupLayout,
}

impl Resource for BindGroupLayout {}

#[derive(Clone, Hash, PartialEq, Eq, Default)]
pub(crate) struct BindGroupLayoutDesc {
    /// Debug label of the bind group layout. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,
    pub entries: Vec<wgpu::BindGroupLayoutEntry>,
}

#[derive(Default)]
pub(crate) struct BindGroupLayoutPool {
    pool: StaticResourcePool<BindGroupLayoutHandle, BindGroupLayoutDesc, BindGroupLayout>,
}

impl BindGroupLayoutPool {
    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &BindGroupLayoutDesc,
    ) -> BindGroupLayoutHandle {
        self.pool.get_or_create(desc, |desc| {
            // TODO(andreas): error handling
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: desc.label.get(),
                entries: &desc.entries,
            });
            BindGroupLayout { layout }
        })
    }

    pub fn get(&self, handle: BindGroupLayoutHandle) -> Result<&BindGroupLayout, PoolError> {
        self.pool.get_resource(handle)
    }
}
