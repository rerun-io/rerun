use slotmap::new_key_type;

use super::resource_pool::*;

new_key_type! { pub(crate) struct BindGroupLayoutHandle; }

pub(crate) struct BindGroupLayout {
    pub(crate) layout: wgpu::BindGroupLayout,
}

impl Resource for BindGroupLayout {
    fn register_use(&self, _current_frame_index: u64) {
        // TODO(andreas): When a bind group layout is last used doesn't tell us all that much since it's needed for pipeline creation only.
        // We need a way to propagate use to dependent resources
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub(crate) struct BindGroupLayoutDesc {
    pub label: String, // TODO(andreas): Ignore for hashing/comparing?
    pub entries: Vec<wgpu::BindGroupLayoutEntry>,
}

pub(crate) struct BindGroupLayoutPool {
    pool: ResourcePool<BindGroupLayoutHandle, BindGroupLayoutDesc, BindGroupLayout>,
}

impl BindGroupLayoutPool {
    pub fn new() -> Self {
        BindGroupLayoutPool {
            pool: ResourcePool::new(),
        }
    }

    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &BindGroupLayoutDesc,
    ) -> BindGroupLayoutHandle {
        self.pool.request(desc, |desc| {
            // TODO(andreas): error handling
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(&desc.label),
                entries: &desc.entries,
            });
            BindGroupLayout { layout }
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool.frame_maintenance(frame_index);
    }

    pub fn get(&self, handle: BindGroupLayoutHandle) -> Result<&BindGroupLayout, PoolError> {
        self.pool.get(handle)
    }
}
