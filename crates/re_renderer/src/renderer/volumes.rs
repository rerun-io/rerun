//! Renderer that makes it easy to draw ray-traced 3D volumes.
//!
//! TODO

use smallvec::smallvec;
use std::num::NonZeroU64;
use wgpu::Face;

use crate::{
    context::uniform_buffer_allocation_size,
    depth_offset::DepthOffset,
    include_file,
    resource_managers::{GpuTexture2DHandle, GpuTexture3DHandle, ResourceManagerError},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BufferDesc, GpuBindGroupHandleStrong,
        GpuBindGroupLayoutHandle, GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc,
        SamplerDesc, ShaderModuleDesc,
    },
    Rgba,
};

use super::{
    DrawData, DrawOrder, FileResolver, FileSystem, RenderContext, Renderer, SharedRendererData,
    WgpuResourcePools,
};

// TODO:
// - need mipmapping to accelerate raytracing

// ---

mod gpu_data {
    use crate::wgpu_buffer_types;

    // - Keep in sync with mirror in volume.wgsl.
    // - See `Volume` for documentation.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct VolumeInfoUBO {
        pub pos_in_world: wgpu_buffer_types::Vec3,
        pub size: wgpu_buffer_types::Vec3,
        pub dimensions: wgpu_buffer_types::UVec3,
    }
}

pub struct Volume {
    /// Top-left corner position in world space.
    pub pos_in_world: glam::Vec3,
    /// The actual world-size of the volume.
    pub size: glam::Vec3,
    /// The dimensions (i.e. number of voxels on each axis) of the volume.
    pub dimensions: glam::UVec3,

    /// The actual 3D texture which contains the data for each voxel.
    pub texture: GpuTexture3DHandle,
}

impl Default for Volume {
    fn default() -> Self {
        Self {
            pos_in_world: glam::Vec3::ZERO,
            size: glam::Vec3::ZERO,
            dimensions: glam::UVec3::ZERO,
            texture: GpuTexture3DHandle::invalid(),
        }
    }
}

#[derive(Clone)]
pub struct VolumeDrawData {
    bind_groups: Vec<GpuBindGroupHandleStrong>,
}

impl DrawData for VolumeDrawData {
    type Renderer = VolumeRenderer;
}

impl VolumeDrawData {
    pub fn new(ctx: &mut RenderContext, volumes: &[Volume]) -> Result<Self, ResourceManagerError> {
        crate::profile_function!();

        let volume_renderer = ctx.renderers.get_or_create::<_, VolumeRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        if volumes.is_empty() {
            return Ok(VolumeDrawData {
                bind_groups: Vec::new(),
            });
        }

        let allocation_size_per_uniform_buffer =
            uniform_buffer_allocation_size::<gpu_data::VolumeInfoUBO>(&ctx.device);
        let combined_buffers_size = allocation_size_per_uniform_buffer * volumes.len() as u64;

        // Allocate all constant buffers at once.
        // TODO(andreas): This should come from a per-frame allocator!
        let volume_info_ubo = ctx.gpu_resources.buffers.alloc(
            &ctx.device,
            &BufferDesc {
                label: "volume_info".into(),
                size: combined_buffers_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            },
        );

        // Fill staging buffer in a separate loop to avoid borrow checker issues
        {
            // TODO(andreas): This should come from a staging buffer.
            let mut staging_buffer = ctx
                .queue
                .write_buffer_with(
                    ctx.gpu_resources
                        .buffers
                        .get_resource(&volume_info_ubo)
                        .unwrap(),
                    0,
                    NonZeroU64::new(combined_buffers_size).unwrap(),
                )
                .unwrap(); // Fails only if mapping is bigger than buffer size.

            for (i, volume) in volumes.iter().enumerate() {
                let offset = i * allocation_size_per_uniform_buffer as usize;

                // CAREFUL: Memory from `write_buffer_with` may not be aligned, causing bytemuck to fail at runtime if we use it to cast the memory to a slice!
                // I.e. this will crash randomly:
                //
                // let target_buffer = bytemuck::from_bytes_mut::<gpu_data::UniformBuffer>(
                //     &mut staging_buffer[offset..(offset + uniform_buffer_size)],
                // );
                //
                // TODO(andreas): with our own staging buffers we could fix this very easily

                staging_buffer[offset..(offset + std::mem::size_of::<gpu_data::VolumeInfoUBO>())]
                    .copy_from_slice(bytemuck::bytes_of(&gpu_data::VolumeInfoUBO {
                        pos_in_world: volume.pos_in_world.into(),
                        size: volume.size.into(),
                        dimensions: volume.dimensions.into(),
                    }));
            }
        }

        let mut bind_groups = Vec::with_capacity(volumes.len());
        for (i, volume) in volumes.iter().enumerate() {
            let texture = ctx.texture_manager_3d.get(&volume.texture)?;

            bind_groups.push(ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &BindGroupDesc {
                    label: "volume".into(),
                    entries: smallvec![
                        BindGroupEntry::Buffer {
                            handle: *volume_info_ubo,
                            offset: i as u64 * allocation_size_per_uniform_buffer,
                            size: NonZeroU64::new(
                                std::mem::size_of::<gpu_data::VolumeInfoUBO>() as u64
                            ),
                        },
                        BindGroupEntry::DefaultTextureView(**texture),
                    ],
                    layout: volume_renderer.bind_group_layout,
                },
                &ctx.gpu_resources.bind_group_layouts,
                &ctx.gpu_resources.textures,
                &ctx.gpu_resources.buffers,
                &ctx.gpu_resources.samplers,
            ));
        }

        Ok(VolumeDrawData { bind_groups })
    }
}

pub struct VolumeRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for VolumeRenderer {
    type RendererDrawData = VolumeDrawData;

    fn create_renderer<Fs: FileSystem>(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> Self {
        crate::profile_function!();

        let bind_group_layout = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "volume_bg_layout".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            // We could use dynamic offset here into a single large buffer.
                            // But we have to set a new texture anyways and its doubtful that splitting the bind group is of any use.
                            has_dynamic_offset: false,
                            min_binding_size: (std::mem::size_of::<gpu_data::VolumeInfoUBO>()
                                as u64)
                                .try_into()
                                .ok(),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            },
        );

        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "volume_rp_layout".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &ShaderModuleDesc {
                label: "volume".into(),
                source: include_file!("../../shader/volume.wgsl"),
            },
        );

        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "volume".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_module,
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(wgpu::ColorTargetState {
                    format: ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    // polygon_mode: wgpu::PolygonMode::Line,
                    cull_mode: None,
                    // cull_mode: Some(Face::Back),
                    ..Default::default()
                },
                // TODO
                depth_stencil: ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE,
                multisample: ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE,
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        VolumeRenderer {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::RendererDrawData,
    ) -> anyhow::Result<()> {
        crate::profile_function!();
        if draw_data.bind_groups.is_empty() {
            return Ok(());
        }

        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;
        pass.set_pipeline(pipeline);

        for bind_group in &draw_data.bind_groups {
            let bind_group = pools.bind_groups.get_resource(bind_group)?;
            pass.set_bind_group(1, bind_group, &[]);
            pass.draw(0..36, 0..1);
        }

        Ok(())
    }

    fn draw_order() -> u32 {
        DrawOrder::Transparent as u32
    }
}
