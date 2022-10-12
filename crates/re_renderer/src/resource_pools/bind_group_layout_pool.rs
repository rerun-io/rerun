use slotmap::new_key_type;

use super::resource_pool::*;

new_key_type! { pub(crate) struct BindGroupLayoutHandle; }

pub(crate) struct BindGroupLayout {
    pub(crate) layout: wgpu::BindGroupLayout,
}

impl Resource for BindGroupLayout {}

#[derive(Clone, Hash, PartialEq, Eq)]
pub(crate) struct BindGroupLayoutDesc {
    /// Debug label of the bind group layout. This will show up in graphics debuggers for easy identification.
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

    pub fn get(&self, handle: BindGroupLayoutHandle) -> Result<&BindGroupLayout, PoolError> {
        self.pool.get(handle)
    }
}
