//! Point renderer for efficient rendering of point clouds.
//!
//!
//! How it works:
//! =================
//! Points are rendered as quads and stenciled out by a fragment shader.
//! Quad spanning happens in the vertex shader, uploaded are only the data for the actual points.
//!
//! Like with the [`super::LineRenderer`], we're rendering as all quads in a single triangle list draw call.
//! (Rationale for this can be found in the LineRenderer's documentation)
//!
//! For WebGL compatibility, data is uploaded as textures. Color is stored in a separate srgb texture, meaning
//! that srgb->linear conversion should be happening on sampling.
//!

use std::num::NonZeroU32;

use bytemuck::Zeroable;

use crate::{
    include_file,
    renderer::utils::next_multiple_of,
    resource_pools::{
        bind_group_layout_pool::{BindGroupLayoutDesc, BindGroupLayoutHandle},
        bind_group_pool::{BindGroupDesc, BindGroupEntry, BindGroupHandle},
        pipeline_layout_pool::PipelineLayoutDesc,
        render_pipeline_pool::*,
        shader_module_pool::ShaderModuleDesc,
    },
    view_builder::ViewBuilder,
};

use super::*;

mod gpu_data {
    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PositionData {
        pub pos: glam::Vec3,
        pub radius: f32, // Might use a half here to free memory for more!
    }
    static_assertions::assert_eq_size!(PositionData, glam::Vec4);
}

/// A point cloud drawing operation.
/// Expected to be recrated every frame.
#[derive(Clone)]
pub struct PointCloudDrawable {
    bind_group: BindGroupHandle,
    num_quads: u32,
}

impl Drawable for PointCloudDrawable {
    type Renderer = PointCloudRenderer;
}

/// Description of a point cloud.
pub struct Point {
    /// Connected points. Must be at least 2.
    pub position: glam::Vec3,

    /// Radius of the line strip in world space
    /// TODO(andreas) Should be able to specify if this is in pixels, or has a minimum width in pixels.
    pub radius: f32,

    /// srgb color. Alpha unused right now
    pub color: [u8; 4],
}

impl PointCloudDrawable {
    pub fn new(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        points: &[Point],
    ) -> anyhow::Result<Self> {
        let line_renderer = ctx.renderers.get_or_create::<PointCloudRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.resource_pools,
            device,
        );

        // Textures are 2D since 1D textures are very limited in size (8k typically).
        // Need to keep these values in sync with lines.wgsl!
        const TEXTURE_SIZE: u32 = 1024; // 1024 x 1024 x (vec4<f32> + [u8;4]) == 20mb, ~1mio points

        // Make sure rows the texture can be copied easily.
        static_assertions::const_assert_eq!(
            TEXTURE_SIZE * std::mem::size_of::<gpu_data::PositionData>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );
        static_assertions::const_assert_eq!(
            TEXTURE_SIZE * std::mem::size_of::<[u8; 4]>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );

        // TODO(andreas): We want a "stack allocation" here that lives for one frame.
        //                  Note also that this doesn't protect against sharing the same texture with several PointDrawable!
        let pos_and_size_texture_desc = wgpu::TextureDescriptor {
            label: Some("point cloud position data"),
            size: wgpu::Extent3d {
                width: TEXTURE_SIZE,
                height: TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        };

        let position_data_texture = ctx
            .resource_pools
            .textures
            .request(device, &pos_and_size_texture_desc);
        let color_texture = ctx.resource_pools.textures.request(
            device,
            &wgpu::TextureDescriptor {
                label: Some("point cloud color data"),
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb, // Declaring this as srgb here saves us manual conversion in the shader!
                ..pos_and_size_texture_desc
            },
        );

        // TODO(andreas): We want a staging-belt(-like) mechanism to upload data instead of the queue.
        //                  These staging buffers would be provided by the belt.
        // To make the data upload simpler (and have it be done in one go), we always update full rows of each of our textures
        let num_points_written = next_multiple_of(points.len() as u32, TEXTURE_SIZE) as usize;
        let num_points_zeroed = num_points_written - points.len();
        let mut position_and_size_staging = Vec::with_capacity(num_points_written);
        position_and_size_staging.extend(points.iter().map(|point| gpu_data::PositionData {
            pos: point.position,
            radius: point.radius,
        }));
        position_and_size_staging
            .extend(std::iter::repeat(gpu_data::PositionData::zeroed()).take(num_points_zeroed));
        let mut color_staging = Vec::with_capacity(num_points_written);
        color_staging.extend(points.iter().map(|point| point.color));
        color_staging.extend(std::iter::repeat([0, 0, 0, 0]).take(num_points_zeroed));

        // Upload data from staging buffers to gpu.
        let size = wgpu::Extent3d {
            width: TEXTURE_SIZE,
            height: num_points_written as u32 / TEXTURE_SIZE,
            depth_or_array_layers: 1,
        };
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &ctx
                    .resource_pools
                    .textures
                    .get(position_data_texture)?
                    .texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&position_and_size_staging),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(
                    TEXTURE_SIZE * std::mem::size_of::<gpu_data::PositionData>() as u32,
                ),
                rows_per_image: None,
            },
            size,
        );
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &ctx.resource_pools.textures.get(color_texture)?.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&color_staging),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(
                    TEXTURE_SIZE * std::mem::size_of::<[u8; 4]>() as u32,
                ),
                rows_per_image: None,
            },
            size,
        );

        Ok(PointCloudDrawable {
            bind_group: ctx.resource_pools.bind_groups.request(
                device,
                &BindGroupDesc {
                    label: "line drawable".into(),
                    entries: vec![
                        BindGroupEntry::TextureView(position_data_texture),
                        BindGroupEntry::TextureView(color_texture),
                    ],
                    layout: line_renderer.bind_group_layout,
                },
                &ctx.resource_pools.bind_group_layouts,
                &ctx.resource_pools.textures,
                &ctx.resource_pools.buffers,
                &ctx.resource_pools.samplers,
            ),
            num_quads: points.len() as _,
        })
    }
}

pub struct PointCloudRenderer {
    render_pipeline: RenderPipelineHandle,
    bind_group_layout: BindGroupLayoutHandle,
}

impl Renderer for PointCloudRenderer {
    type DrawData = PointCloudDrawable;

    fn create_renderer(
        shared_data: &SharedRendererData,
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
    ) -> Self {
        let bind_group_layout = pools.bind_group_layouts.request(
            device,
            &BindGroupLayoutDesc {
                label: "point cloud".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
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

        let pipeline_layout = pools.pipeline_layouts.request(
            device,
            &PipelineLayoutDesc {
                label: "point cloud".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.request(
            device,
            &ShaderModuleDesc {
                label: "point cloud".into(),
                source: include_file!("../../shader/point_cloud.wgsl"),
            },
        );

        let render_pipeline = pools.render_pipelines.request(
            device,
            &RenderPipelineDesc {
                label: "point cloud".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_module,

                // Instance buffer with pairwise overlapping instances!
                vertex_buffers: vec![],
                render_targets: vec![Some(ViewBuilder::FORMAT_HDR.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: ViewBuilder::FORMAT_DEPTH,
                    depth_compare: wgpu::CompareFunction::Greater,
                    depth_write_enabled: true,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
            },
            &pools.pipeline_layouts,
            &pools.shader_modules,
        );

        PointCloudRenderer {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::DrawData,
    ) -> anyhow::Result<()> {
        let pipeline = pools.render_pipelines.get(self.render_pipeline)?;
        let bind_group = pools.bind_groups.get(draw_data.bind_group)?;

        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(1, &bind_group.bind_group, &[]);
        pass.draw(0..draw_data.num_quads * 6, 0..1);

        Ok(())
    }
}
