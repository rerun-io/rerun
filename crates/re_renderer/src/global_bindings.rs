use crate::{
    wgpu_buffer_types,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroupHandleStrong,
        GpuBindGroupLayoutHandle, GpuBufferHandleStrong, GpuSamplerHandle, SamplerDesc,
        WgpuResourcePools,
    },
};

use bytemuck::{Pod, Zeroable};
use smallvec::smallvec;

/// Mirrors the GPU contents of a frame-global uniform buffer.
///
/// Contains information that is constant for a single frame like camera.
/// (does not contain information that is special to a particular renderer)
#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub(crate) struct FrameUniformBuffer {
    pub view_from_world: wgpu_buffer_types::Mat4,
    pub world_from_view: wgpu_buffer_types::Mat4,
    pub projection_from_view: wgpu_buffer_types::Mat4,
    pub projection_from_world: wgpu_buffer_types::Mat4,

    /// Camera position in world space.
    pub camera_position: glam::Vec3,

    /// For perspective: Multiply this with a camera distance to get a measure of how wide a pixel is in world units.
    /// For orthographic: This is the world size value, independent of distance.
    pub pixel_world_size_from_camera_distance: f32,

    /// Camera direction in world space.
    /// Same as `-view_from_world.row(2).truncate()`
    pub camera_forward: glam::Vec3,

    /// How many pixels there are per point.
    /// I.e. the ui scaling factor.
    pub pixels_from_point: f32,

    /// (tan(fov_y / 2) * aspect_ratio, tan(fov_y /2)), i.e. half ratio of screen dimension to screen distance in x & y.
    /// Both values are set to positive infinity for orthographic projection
    pub tan_half_fov: wgpu_buffer_types::Vec2,

    // Size used for all point radii given with Size::AUTO.
    pub auto_size_points: f32,

    // Size used for all line radii given with Size::AUTO.
    pub auto_size_lines: f32,

    /// Factor used to compute depth offsets, see `depth_offset.wgsl`.
    pub depth_offset_factor: f32,
    pub _padding: glam::Vec3,
}

pub(crate) struct GlobalBindings {
    pub(crate) layout: GpuBindGroupLayoutHandle,
    nearest_neighbor_sampler: GpuSamplerHandle,
    trilinear_sampler: GpuSamplerHandle,
}

impl GlobalBindings {
    pub fn new(pools: &mut WgpuResourcePools, device: &wgpu::Device) -> Self {
        Self {
            layout: pools.bind_group_layouts.get_or_create(
                device,
                &BindGroupLayoutDesc {
                    label: "global bind group layout".into(),

                    // Needs to be kept in sync with `global_bindings.wgsl` / `create_bind_group`
                    entries: vec![
                        // The global per-frame uniform buffer.
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::all(),
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: (std::mem::size_of::<FrameUniformBuffer>()
                                    as u64)
                                    .try_into()
                                    .ok(),
                            },
                            count: None,
                        },
                        // Sampler without any filtering.
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                            count: None,
                        },
                        // Trilinear sampler.
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                },
            ),
            nearest_neighbor_sampler: pools.samplers.get_or_create(
                device,
                &SamplerDesc {
                    label: "nearest".into(),
                    address_mode_u: wgpu::AddressMode::Repeat,
                    address_mode_v: wgpu::AddressMode::Repeat,
                    address_mode_w: wgpu::AddressMode::Repeat,
                    ..Default::default()
                },
            ),
            trilinear_sampler: pools.samplers.get_or_create(
                device,
                &SamplerDesc {
                    label: "linear".into(),
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    address_mode_u: wgpu::AddressMode::Repeat,
                    address_mode_v: wgpu::AddressMode::Repeat,
                    address_mode_w: wgpu::AddressMode::Repeat,
                    ..Default::default()
                },
            ),
        }
    }

    /// Creates a bind group that follows the global bind group layout.
    pub fn create_bind_group(
        &self,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        frame_uniform_buffer: &GpuBufferHandleStrong,
    ) -> GpuBindGroupHandleStrong {
        pools.bind_groups.alloc(
            device,
            // Needs to be kept in sync with `global_bindings.wgsl` / `self.layout`
            &BindGroupDesc {
                label: "global bind group".into(),
                entries: smallvec![
                    BindGroupEntry::Buffer {
                        handle: **frame_uniform_buffer,
                        offset: 0,
                        size: None,
                    },
                    BindGroupEntry::Sampler(self.nearest_neighbor_sampler),
                    BindGroupEntry::Sampler(self.trilinear_sampler),
                ],
                layout: self.layout,
            },
            &pools.bind_group_layouts,
            &pools.textures,
            &pools.buffers,
            &pools.samplers,
        )
    }
}
