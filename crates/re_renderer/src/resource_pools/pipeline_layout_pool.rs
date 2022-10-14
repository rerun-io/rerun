use crate::debug_label::DebugLabel;

use super::{bind_group_layout_pool::*, resource_pool::*};

slotmap::new_key_type! { pub(crate) struct PipelineLayoutHandle; }

pub(crate) struct PipelineLayout {
    pub(crate) layout: wgpu::PipelineLayout,
}

impl Resource for PipelineLayout {}

#[derive(Clone, Hash, PartialEq, Eq)]
pub(crate) struct PipelineLayoutDesc {
    /// Debug label of the pipeline layout. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,
    // TODO(andreas) use SmallVec or similar, limited to 4
    pub entries: Vec<BindGroupLayoutHandle>,
}

#[derive(Default)]
pub(crate) struct PipelineLayoutPool {
    pool: ResourcePool<PipelineLayoutHandle, PipelineLayoutDesc, PipelineLayout>,
}

impl PipelineLayoutPool {
    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &PipelineLayoutDesc,
        bind_group_layout_pool: &BindGroupLayoutPool,
    ) -> PipelineLayoutHandle {
        self.pool.get_handle(desc, |desc| {
            // TODO(andreas): error handling
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: desc.label.get(),
                bind_group_layouts: &desc
                    .entries
                    .iter()
                    .map(|handle| &bind_group_layout_pool.get(*handle).unwrap().layout)
                    .collect::<Vec<_>>(),
                push_constant_ranges: &[], // Sadly not widely supported
            });
            PipelineLayout { layout }
        })
    }

    pub fn get(&self, handle: PipelineLayoutHandle) -> Result<&PipelineLayout, PoolError> {
        self.pool.get_resource(handle)
    }
}
