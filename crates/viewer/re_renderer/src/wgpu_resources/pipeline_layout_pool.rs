use super::bind_group_layout_pool::GpuBindGroupLayoutHandle;
use super::static_resource_pool::{StaticResourcePool, StaticResourcePoolReadLockAccessor};
use crate::RenderContext;
use crate::debug_label::DebugLabel;

slotmap::new_key_type! { pub struct GpuPipelineLayoutHandle; }

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PipelineLayoutDesc {
    /// Debug label of the pipeline layout. This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,
    // TODO(andreas) use SmallVec or similar, limited to 4
    pub entries: Vec<GpuBindGroupLayoutHandle>,
}

#[derive(Default)]
pub struct GpuPipelineLayoutPool {
    pool: StaticResourcePool<GpuPipelineLayoutHandle, PipelineLayoutDesc, wgpu::PipelineLayout>,
}

impl GpuPipelineLayoutPool {
    pub fn get_or_create(
        &self,
        ctx: &RenderContext,
        desc: &PipelineLayoutDesc,
    ) -> GpuPipelineLayoutHandle {
        self.pool.get_or_create(desc, |desc| {
            // TODO(andreas): error handling

            let bind_groups = ctx.gpu_resources.bind_group_layouts.resources();

            ctx.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: desc.label.get(),
                    bind_group_layouts: &desc
                        .entries
                        .iter()
                        .map(|handle| bind_groups.get(*handle).unwrap())
                        .collect::<Vec<_>>(),
                    push_constant_ranges: &[], // Sadly not widely supported
                })
        })
    }

    /// Locks the resource pool for resolving handles.
    ///
    /// While it is locked, no new resources can be added.
    pub fn resources(
        &self,
    ) -> StaticResourcePoolReadLockAccessor<'_, GpuPipelineLayoutHandle, wgpu::PipelineLayout> {
        self.pool.resources()
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        self.pool.current_frame_index = frame_index;
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }
}
