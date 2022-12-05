//! Point renderer for efficient rendering of point clouds.
//!
//!
//! How it works:
//! =================
//! Points are rendered as quads and stenciled out by a fragment shader.
//! Quad spanning happens in the vertex shader, uploaded are only the data for the actual points (no vertex buffer!).
//!
//! Like with the `super::lines::LineRenderer`, we're rendering as all quads in a single triangle list draw call.
//! (Rationale for this can be found in the [`lines.rs`]'s documentation)
//!
//! For WebGL compatibility, data is uploaded as textures. Color is stored in a separate srgb texture, meaning
//! that srgb->linear conversion happens on texture load.
//!

use std::num::NonZeroU32;

use bytemuck::Zeroable;
use itertools::Itertools;
use smallvec::smallvec;

use crate::{
    include_file,
    renderer::utils::next_multiple_of,
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroupHandleStrong,
        GpuBindGroupLayoutHandle, GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc,
        ShaderModuleDesc, TextureDesc,
    },
};

use super::*;

mod gpu_data {
    // Don't use `wgsl_buffer_types` since none of this data goes into a buffer, so its alignment rules don't apply.

    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PositionData {
        pub pos: glam::Vec3,
        pub radius: f32, // Might use a f16 here to free memory for more data!
    }
    static_assertions::assert_eq_size!(PositionData, glam::Vec4);
}

/// A point cloud drawing operation.
/// Expected to be recrated every frame.
#[derive(Clone)]
pub struct PointCloudDrawData {
    bind_group: Option<GpuBindGroupHandleStrong>,
    num_quads: u32,
}

impl DrawData for PointCloudDrawData {
    type Renderer = PointCloudRenderer;
}

/// Description of a point cloud.
pub struct PointCloudPoint {
    /// Connected points. Must be at least 2.
    pub position: glam::Vec3,

    /// Radius of the point in world space
    /// TODO(andreas) Should be able to specify if this is in pixels, or has a minimum width in pixels.
    pub radius: f32,

    /// The points color in srgb color space. Alpha unused right now
    pub srgb_color: [u8; 4],
}

impl PointCloudDrawData {
    /// Transforms and uploads point cloud data to be consumed by gpu.
    ///
    /// Try to bundle all points into a single draw data instance whenever possible.
    ///
    /// If you pass point instances, None will be returned.
    pub fn new(ctx: &mut RenderContext, points: &[PointCloudPoint]) -> anyhow::Result<Self> {
        crate::profile_function!();

        let point_renderer = ctx.renderers.get_or_create::<_, PointCloudRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        if points.is_empty() {
            return Ok(PointCloudDrawData {
                bind_group: None,
                num_quads: 0,
            });
        }

        // Textures are 2D since 1D textures are very limited in size (8k typically).
        // Need to keep this value in sync with point_cloud.wgsl!
        const TEXTURE_SIZE: u32 = 1024; // 1024 x 1024 x (vec4<f32> + [u8;4]) == 20mb, ~1mio points

        // Make sure the size of a row is a multiple of the row byte alignment to make buffer copies easier.
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

        // TODO(andreas) split up point cloud into several textures when that happens.
        anyhow::ensure!(
            points.len() <= (TEXTURE_SIZE * TEXTURE_SIZE) as usize,
            "Current maximum number of points supported for a point cloud is {}",
            TEXTURE_SIZE * TEXTURE_SIZE
        );

        // TODO(andreas): We want a "stack allocation" here that lives for one frame.
        //                  Note also that this doesn't protect against sharing the same texture with several PointDrawData!
        let position_data_texture_desc = TextureDesc {
            label: "point cloud position data".into(),
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
            .gpu_resources
            .textures
            .alloc(&ctx.device, &position_data_texture_desc);
        let color_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: "point cloud color data".into(),
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb, // Declaring this as srgb here saves us manual conversion in the shader!
                ..position_data_texture_desc
            },
        );

        // TODO(andreas): We want a staging-belt(-like) mechanism to upload data instead of the queue.
        //                  These staging buffers would be provided by the belt.
        // To make the data upload simpler (and have it be done in one go), we always update full rows of each of our textures
        let num_points_written = next_multiple_of(points.len() as u32, TEXTURE_SIZE) as usize;
        let num_points_zeroed = num_points_written - points.len();
        let position_and_size_staging = {
            crate::profile_scope!("collect_pos_size");
            points
                .iter()
                .map(|point| gpu_data::PositionData {
                    pos: point.position,
                    radius: point.radius,
                })
                .chain(std::iter::repeat(gpu_data::PositionData::zeroed()).take(num_points_zeroed))
                .collect_vec()
        };

        let color_staging = {
            crate::profile_scope!("collect_colors");
            points
                .iter()
                .map(|point| point.srgb_color)
                .chain(std::iter::repeat([0, 0, 0, 0]).take(num_points_zeroed))
                .collect_vec()
        };

        // Upload data from staging buffers to gpu.
        let size = wgpu::Extent3d {
            width: TEXTURE_SIZE,
            height: num_points_written as u32 / TEXTURE_SIZE,
            depth_or_array_layers: 1,
        };

        {
            crate::profile_scope!("write_pos_size_texture");
            ctx.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &ctx
                        .gpu_resources
                        .textures
                        .get_resource(&position_data_texture)?
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
        }

        {
            crate::profile_scope!("write_color_texture");
            ctx.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &ctx
                        .gpu_resources
                        .textures
                        .get_resource(&color_texture)?
                        .texture,
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
        }

        Ok(PointCloudDrawData {
            bind_group: Some(ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &BindGroupDesc {
                    label: "line drawdata".into(),
                    entries: smallvec![
                        BindGroupEntry::DefaultTextureView(*position_data_texture),
                        BindGroupEntry::DefaultTextureView(*color_texture),
                    ],
                    layout: point_renderer.bind_group_layout,
                },
                &ctx.gpu_resources.bind_group_layouts,
                &ctx.gpu_resources.textures,
                &ctx.gpu_resources.buffers,
                &ctx.gpu_resources.samplers,
            )),
            num_quads: points.len() as _,
        })
    }
}

pub struct PointCloudRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for PointCloudRenderer {
    type RendererDrawData = PointCloudDrawData;

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

        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "point cloud".into(),
                entries: vec![shared_data.global_bindings.layout, bind_group_layout],
            },
            &pools.bind_group_layouts,
        );

        let shader_module = pools.shader_modules.get_or_create(
            device,
            resolver,
            &ShaderModuleDesc {
                label: "point cloud".into(),
                source: include_file!("../../shader/point_cloud.wgsl"),
            },
        );

        let render_pipeline = pools.render_pipelines.get_or_create(
            device,
            &RenderPipelineDesc {
                label: "point cloud".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_module,
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE,
                multisample: wgpu::MultisampleState {
                    // We discard pixels to do the round cutout, therefore we need to calculate our own sampling mask.
                    alpha_to_coverage_enabled: true,
                    ..ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE
                },
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
        draw_data: &Self::RendererDrawData,
    ) -> anyhow::Result<()> {
        crate::profile_function!();
        let Some(bind_group) = &draw_data.bind_group else {
            return Ok(()) // Empty drawdata
        };

        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;
        let bind_group = pools.bind_groups.get_resource(bind_group)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, bind_group, &[]);
        pass.draw(0..draw_data.num_quads * 6, 0..1);

        Ok(())
    }
}
