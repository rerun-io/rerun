use crate::debug_label::DebugLabel;

use super::{bind_group_layout_pool::*, resource::*, static_resource_pool::*};

slotmap::new_key_type! { pub struct GpuPipelineLayoutHandle; }

pub struct GpuPipelineLayout {
    pub layout: wgpu::PipelineLayout,
}

impl GpuResource for GpuPipelineLayout {}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct PipelineLayoutDesc {
    /// Debug label of the pipeline layout. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,
    // TODO(andreas) use SmallVec or similar, limited to 4
    pub entries: Vec<GpuBindGroupLayoutHandle>,
}

#[derive(Default)]
pub struct GpuPipelineLayoutPool {
    pool: StaticResourcePool<GpuPipelineLayoutHandle, PipelineLayoutDesc, GpuPipelineLayout>,
}

impl GpuPipelineLayoutPool {
    pub fn get_or_create(
        &mut self,
        device: &wgpu::Device,
        desc: &PipelineLayoutDesc,
        bind_group_layout_pool: &GpuBindGroupLayoutPool,
    ) -> GpuPipelineLayoutHandle {
        self.pool.get_or_create(desc, |desc| {
            // TODO(andreas): error handling
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: desc.label.get(),
                bind_group_layouts: &desc
                    .entries
                    .iter()
                    .map(|handle| &bind_group_layout_pool.get_resource(*handle).unwrap().layout)
                    .collect::<Vec<_>>(),
                push_constant_ranges: &[], // Sadly not widely supported
            });
            GpuPipelineLayout { layout }
        })
    }

    pub fn get_resource(
        &self,
        handle: GpuPipelineLayoutHandle,
    ) -> Result<&GpuPipelineLayout, PoolError> {
        self.pool.get_resource(handle)
    }
}
