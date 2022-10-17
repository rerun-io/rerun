use crate::resource_pools::{
    bind_group_layout_pool::*,
    bind_group_pool::*,
    sampler_pool::{SamplerDesc, SamplerHandle},
    WgpuResourcePools,
};

/// Mirrors the GPU contents of a frame-global uniform buffer.
/// Contains information that is constant for a single frame like camera.
/// (does not contain information that is special to a particular renderer or global to the Context)
//#[repr(C)]
// pub(crate) struct FrameUniformBuffer {
//     //TODO(andreas): camera matrix and the like.
// }

pub(crate) struct GlobalBindings {
    pub(crate) layout: BindGroupLayoutHandle,
    nearest_neighbor_sampler: SamplerHandle,
}

impl GlobalBindings {
    pub fn new(pools: &mut WgpuResourcePools, device: &wgpu::Device) -> Self {
        Self {
            layout: pools.bind_group_layouts.request(
                device,
                &BindGroupLayoutDesc {
                    label: "global bind group layout".into(),

                    entries: vec![
                        // The global per-frame uniform buffer.
                        // wgpu::BindGroupLayoutEntry {
                        //     binding: 0,
                        //     visibility: wgpu::ShaderStages::all(),
                        //     ty: wgpu::BindingType::Buffer {
                        //         ty: wgpu::BufferBindingType::Uniform,
                        //         has_dynamic_offset: false,
                        //         min_binding_size: NonZeroU64::new(
                        //             std::mem::size_of::<FrameUniformBuffer>() as _,
                        //         ),
                        //     },
                        //     count: None,
                        // },
                        // Sampler without any filtering.
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                            count: None,
                        },
                    ],
                },
            ),
            nearest_neighbor_sampler: pools.samplers.request(
                device,
                &SamplerDesc {
                    label: "nearest".into(),
                    ..Default::default()
                },
            ),
        }
    }

    /// Creates a bind group matching for the bind group layout defined by [`create_global_bind_group_layout`]
    pub fn create_bind_group(
        &self,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> BindGroupHandle {
        pools.bind_groups.request(
            device,
            &BindGroupDesc {
                label: "global bind group".into(),
                entries: vec![
                    //BindGroupEntry::TextureView(data.hdr_target),
                    BindGroupEntry::Sampler(self.nearest_neighbor_sampler),
                ],
                layout: self.layout,
            },
            &pools.bind_group_layouts,
            &pools.textures,
            &pools.samplers,
        )
    }
}
