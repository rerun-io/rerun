use slotmap::new_key_type;

use super::{bind_group_layout_pool::*, resource_pool::*};

new_key_type! { pub(crate) struct PipelineLayoutHandle; }

pub(crate) struct PipelineLayout {
    pub(crate) layout: wgpu::PipelineLayout,
}

impl Resource for PipelineLayout {
    fn register_use(&self, _current_frame_index: u64) {
        // TODO(andreas): When a pipeline layout is last used doesn't tell us all that much since it's needed for pipeline creation only.
        // We need a way to propagate use to dependent resources
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub(crate) struct PipelineLayoutDesc {
    /// Debug label of the pipeline layout. This will show up in graphics debuggers for easy identification.
    pub label: String, // TODO(andreas): Ignore for hashing/comparing?
    pub entries: Vec<BindGroupLayoutHandle>,
}

pub(crate) struct PipelineLayoutPool {
    pool: ResourcePool<PipelineLayoutHandle, PipelineLayoutDesc, PipelineLayout>,
}

impl PipelineLayoutPool {
    pub fn new() -> Self {
        PipelineLayoutPool {
            pool: ResourcePool::new(),
        }
    }

    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &PipelineLayoutDesc,
        bind_group_layout_pool: &BindGroupLayoutPool,
    ) -> PipelineLayoutHandle {
        self.pool.request(desc, |desc| {
            // TODO(andreas): error handling
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&desc.label),
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

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool.frame_maintenance(frame_index);
    }

    pub fn get(&self, handle: PipelineLayoutHandle) -> Result<&PipelineLayout, PoolError> {
        self.pool.get(handle)
    }
}
