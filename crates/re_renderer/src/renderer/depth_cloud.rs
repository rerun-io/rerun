//! Renderer that makes it easy to draw ray-traced 3D depth_clouds from depth textures.
//!
//! TODO (probably gonna look like the docs for Rectangles)

use itertools::Itertools;
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
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BufferDesc, GpuBindGroup,
        GpuBindGroupLayoutHandle, GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc,
        SamplerDesc, ShaderModuleDesc, TextureDesc,
    },
    Rgba,
};

use super::{
    DrawData, DrawOrder, FileResolver, FileSystem, RenderContext, Renderer, SharedRendererData,
    WgpuResourcePools,
};

// ---

mod gpu_data {
    // - Keep in sync with mirror in depth_cloud.wgsl.
    // - See `DepthCloud` for documentation.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DepthCloudInfoUBO {
        pub world_from_model: glam::Mat4,
        pub model_from_world: glam::Mat4,
    }
}

pub struct DepthCloud {
    pub world_from_model: glam::Mat4,
    pub model_from_world: glam::Mat4,

    pub depth_dimensions: glam::UVec2,
    pub depth_data: Vec<f32>,
}

impl Default for DepthCloud {
    fn default() -> Self {
        Self {
            world_from_model: glam::Mat4::IDENTITY,
            model_from_world: glam::Mat4::IDENTITY,
            depth_dimensions: glam::UVec2::ZERO,
            depth_data: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct DepthCloudDrawData {
    // Every single point cloud, its total number of points.
    bind_groups: Vec<(u32, GpuBindGroup)>,
}

impl DrawData for DepthCloudDrawData {
    type Renderer = DepthCloudRenderer;
}

impl DepthCloudDrawData {
    pub fn new(
        ctx: &mut RenderContext,
        depth_clouds: &[DepthCloud],
    ) -> Result<Self, ResourceManagerError> {
        crate::profile_function!();

        let depth_cloud_renderer = ctx.renderers.get_or_create::<_, DepthCloudRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        if depth_clouds.is_empty() {
            return Ok(DepthCloudDrawData {
                bind_groups: Vec::new(),
            });
        }

        let allocation_size_per_uniform_buffer =
            uniform_buffer_allocation_size::<gpu_data::DepthCloudInfoUBO>(&ctx.device);
        let combined_buffers_size = allocation_size_per_uniform_buffer * depth_clouds.len() as u64;

        // Allocate all constant buffers at once.
        // TODO: use https://github.com/rerun-io/rerun/pull/1400, maybe even ship on top of that PR
        // directly.
        let depth_cloud_info_ubo = ctx.gpu_resources.buffers.alloc(
            &ctx.device,
            &BufferDesc {
                label: "depth_cloud_info".into(),
                size: combined_buffers_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
                mapped_at_creation: false,
            },
        );

        // Fill staging buffer in a separate loop to avoid borrow checker issues
        {
            // TODO(andreas): This should come from a staging buffer.
            let mut staging_buffer = ctx
                .queue
                .write_buffer_with(
                    &depth_cloud_info_ubo.inner,
                    0,
                    NonZeroU64::new(combined_buffers_size).unwrap(),
                )
                .unwrap(); // Fails only if mapping is bigger than buffer size.

            for (i, depth_cloud) in depth_clouds.iter().enumerate() {
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

                staging_buffer
                    [offset..(offset + std::mem::size_of::<gpu_data::DepthCloudInfoUBO>())]
                    .copy_from_slice(bytemuck::bytes_of(&gpu_data::DepthCloudInfoUBO {
                        world_from_model: depth_cloud.world_from_model,
                        model_from_world: depth_cloud.model_from_world,
                    }));
            }
        }

        let mut bind_groups = Vec::with_capacity(depth_clouds.len());
        for (i, depth_cloud) in depth_clouds.iter().enumerate() {
            let depth_texture = {
                crate::profile_scope!("depth");

                let depth_texture_size = wgpu::Extent3d {
                    width: depth_cloud.depth_dimensions.x,
                    height: depth_cloud.depth_dimensions.y,
                    depth_or_array_layers: 1,
                };
                let depth_texture_desc = TextureDesc {
                    label: "depth texture".into(),
                    size: depth_texture_size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                };
                let depth_texture = ctx
                    .gpu_resources
                    .textures
                    .alloc(&ctx.device, &depth_texture_desc);

                let format_info = depth_texture_desc.format.describe();
                let width_blocks =
                    depth_cloud.depth_dimensions.x / format_info.block_dimensions.0 as u32;
                let bytes_per_row_unaligned = width_blocks * format_info.block_size as u32;

                {
                    crate::profile_scope!("write depth texture");
                    ctx.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture: &depth_texture.inner.texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        bytemuck::cast_slice(depth_cloud.depth_data.as_slice()),
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

            bind_groups.push((
                depth_cloud.depth_dimensions.x * depth_cloud.depth_dimensions.y * 6,
                ctx.gpu_resources.bind_groups.alloc(
                    &ctx.device,
                    &BindGroupDesc {
                        label: "depth_cloud".into(),
                        entries: smallvec![
                            BindGroupEntry::Buffer {
                                handle: depth_cloud_info_ubo.handle,
                                offset: i as u64 * allocation_size_per_uniform_buffer,
                                size: NonZeroU64::new(
                                    std::mem::size_of::<gpu_data::DepthCloudInfoUBO>() as u64
                                ),
                            },
                            BindGroupEntry::DefaultTextureView(depth_texture.handle),
                        ],
                        layout: depth_cloud_renderer.bind_group_layout,
                    },
                    &ctx.gpu_resources.bind_group_layouts,
                    &ctx.gpu_resources.textures,
                    &ctx.gpu_resources.buffers,
                    &ctx.gpu_resources.samplers,
                ),
            ));
        }

        Ok(DepthCloudDrawData { bind_groups })
    }
}

pub struct DepthCloudRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for DepthCloudRenderer {
    type RendererDrawData = DepthCloudDrawData;

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
                label: "depth_cloud_bg_layout".into(),
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
                            min_binding_size: (std::mem::size_of::<gpu_data::DepthCloudInfoUBO>()
                                as u64)
                                .try_into()
                                .ok(),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
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
                label: "depth_cloud_rp_layout".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &ShaderModuleDesc {
                label: "depth_cloud".into(),
                source: include_file!("../../shader/depth_cloud.wgsl"),
            },
        );

        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "depth_cloud".into(),
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

        DepthCloudRenderer {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> anyhow::Result<()> {
        crate::profile_function!();
        if draw_data.bind_groups.is_empty() {
            return Ok(());
        }

        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;
        pass.set_pipeline(pipeline);

        for (num_points, bind_group) in &draw_data.bind_groups {
            pass.set_bind_group(1, bind_group, &[]);
            pass.draw(0..*num_points, 0..1);
        }

        Ok(())
    }

    fn draw_order() -> u32 {
        DrawOrder::Transparent as u32
    }
}
