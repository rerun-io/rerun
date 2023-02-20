//! Renderer that makes it easy to draw ray-traced 3D volumes from depth textures.
//!
//! TODO

use smallvec::smallvec;
use std::num::{NonZeroU32, NonZeroU64};
use wgpu::Face;

use crate::{
    context::uniform_buffer_allocation_size,
    depth_offset::DepthOffset,
    include_file,
    resource_managers::{GpuTexture2DHandle, ResourceManagerError},
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BufferDesc, GpuBindGroupHandleStrong,
        GpuBindGroupLayoutHandle, GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc,
        SamplerDesc, ShaderModuleDesc, TextureDesc,
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
        pub world_from_model: glam::Mat4,
        pub model_from_world: glam::Mat4,
        pub dimensions: wgpu_buffer_types::UVec3,
    }
}

pub struct Volume {
    pub world_from_model: glam::Mat4,
    pub model_from_world: glam::Mat4,

    /// The dimensions (i.e. number of voxels on each axis) of the volume.
    pub dimensions: glam::UVec3,

    pub depth_dimensions: glam::UVec2,
    pub depth_data: Vec<f32>,

    pub albedo_dimensions: glam::UVec2,
    pub albedo_data: Vec<u8>,
}

impl Default for Volume {
    fn default() -> Self {
        Self {
            world_from_model: glam::Mat4::IDENTITY,
            model_from_world: glam::Mat4::IDENTITY,
            dimensions: glam::UVec3::ZERO,
            depth_dimensions: glam::UVec2::ZERO,
            depth_data: Vec::new(),
            albedo_dimensions: glam::UVec2::ZERO,
            albedo_data: Vec::new(),
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

                // CAREFUL: Memory from `write_buffer_with` may not be aligned, causing bytemuck
                // to fail at runtime if we use it to cast the memory to a slice!
                // I.e. this will crash randomly:
                //
                // let target_buffer = bytemuck::from_bytes_mut::<gpu_data::UniformBuffer>(
                //     &mut staging_buffer[offset..(offset + uniform_buffer_size)],
                // );
                //
                // TODO(andreas): with our own staging buffers we could fix this very easily

                staging_buffer[offset..(offset + std::mem::size_of::<gpu_data::VolumeInfoUBO>())]
                    .copy_from_slice(bytemuck::bytes_of(&gpu_data::VolumeInfoUBO {
                        world_from_model: volume.world_from_model,
                        model_from_world: volume.model_from_world,
                        dimensions: volume.dimensions.into(),
                    }));
            }
        }

        let mut bind_groups = Vec::with_capacity(volumes.len());
        for (i, volume) in volumes.iter().enumerate() {
            let depth_texture = {
                crate::profile_scope!("depth");

                let depth_texture_size = wgpu::Extent3d {
                    width: volume.depth_dimensions.x,
                    height: volume.depth_dimensions.y,
                    depth_or_array_layers: 1,
                };
                let depth_texture_desc = TextureDesc {
                    label: "depth texture".into(),
                    size: depth_texture_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R32Float, // TODO
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                };
                let depth_texture = ctx
                    .gpu_resources
                    .textures
                    .alloc(&ctx.device, &depth_texture_desc);

                let format_info = depth_texture_desc.format.describe();
                let width_blocks =
                    volume.depth_dimensions.x / format_info.block_dimensions.0 as u32;
                let bytes_per_row_unaligned = width_blocks * format_info.block_size as u32;

                {
                    crate::profile_scope!("write depth texture");
                    ctx.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture: &ctx
                                .gpu_resources
                                .textures
                                .get_resource(&depth_texture)
                                .unwrap()
                                .texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        bytemuck::cast_slice(volume.depth_data.as_slice()),
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(
                                NonZeroU32::new(bytes_per_row_unaligned)
                                    .expect("invalid bytes per row"),
                            ),
                            rows_per_image: None,
                        },
                        depth_texture_size,
                    );
                }

                depth_texture
            };

            let albedo_texture = {
                crate::profile_scope!("albedo");

                let albedo_texture_size = wgpu::Extent3d {
                    width: volume.albedo_dimensions.x,
                    height: volume.albedo_dimensions.y,
                    depth_or_array_layers: 1,
                };
                let albedo_texture_desc = TextureDesc {
                    label: "albedo texture".into(),
                    size: albedo_texture_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm, // TODO
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                };
                let albedo_texture = ctx
                    .gpu_resources
                    .textures
                    .alloc(&ctx.device, &albedo_texture_desc);

                let format_info = albedo_texture_desc.format.describe();
                let width_blocks =
                    volume.albedo_dimensions.x / format_info.block_dimensions.0 as u32;
                let bytes_per_row_unaligned = width_blocks * format_info.block_size as u32;

                {
                    crate::profile_scope!("write albedo texture");
                    ctx.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture: &ctx
                                .gpu_resources
                                .textures
                                .get_resource(&albedo_texture)
                                .unwrap()
                                .texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        bytemuck::cast_slice(volume.albedo_data.as_slice()),
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(
                                NonZeroU32::new(bytes_per_row_unaligned)
                                    .expect("invalid bytes per row"),
                            ),
                            rows_per_image: None,
                        },
                        albedo_texture_size,
                    );
                }

                albedo_texture
            };

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
                        BindGroupEntry::DefaultTextureView(*depth_texture),
                        BindGroupEntry::DefaultTextureView(*albedo_texture),
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
                            // But we have to set a new texture anyways and its doubtful that
                            // splitting the bind group is of any use.
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
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
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
                source: include_file!("../../shader/volume2d.wgsl"),
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
