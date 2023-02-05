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

use std::{
    num::{NonZeroU32, NonZeroU64},
    ops::Range,
};

use crate::{
    context::uniform_buffer_allocation_size, wgpu_resources::BufferDesc, Color32, DebugLabel,
};
use bitflags::bitflags;
use bytemuck::Zeroable;
use itertools::Itertools;
use smallvec::smallvec;

use crate::{
    include_file,
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroupHandleStrong,
        GpuBindGroupLayoutHandle, GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc,
        ShaderModuleDesc, TextureDesc,
    },
    Size,
};

use super::{
    DrawData, FileResolver, FileSystem, RenderContext, Renderer, SharedRendererData,
    WgpuResourcePools,
};

bitflags! {
    /// Property flags for a point batch
    ///
    /// Needs to be kept in sync with `point_cloud.wgsl`
    #[repr(C)]
    #[derive(Default, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PointCloudBatchFlags : u32 {
        /// If true, we shade all points in the batch like spheres.
        const ENABLE_SHADING = 0b0001;
    }
}

mod gpu_data {
    use super::PointCloudBatchFlags;
    use crate::{wgpu_buffer_types, Size};

    // Don't use `wgsl_buffer_types` since this data doesn't go into a buffer, so alignment rules don't apply like on buffers..
    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PositionData {
        pub pos: glam::Vec3,
        pub radius: Size, // Might use a f16 here to free memory for more data!
    }
    static_assertions::assert_eq_size!(PositionData, glam::Vec4);

    /// Uniform buffer that changes for every batch of line strips.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BatchUniformBuffer {
        pub world_from_obj: wgpu_buffer_types::Mat4,
        pub flags: PointCloudBatchFlags,
        pub _padding: glam::Vec3,
    }
}

/// Internal, ready to draw representation of [`PointCloudBatchInfo`]
#[derive(Clone)]
struct PointCloudBatch {
    bind_group: GpuBindGroupHandleStrong,
    vertex_range: Range<u32>,
}

/// A point cloud drawing operation.
/// Expected to be recrated every frame.
#[derive(Clone)]
pub struct PointCloudDrawData {
    bind_group_all_points: Option<GpuBindGroupHandleStrong>,
    batches: Vec<PointCloudBatch>,
}

impl DrawData for PointCloudDrawData {
    type Renderer = PointCloudRenderer;
}

/// Data that is valid for a batch of point cloud points.
pub struct PointCloudBatchInfo {
    pub label: DebugLabel,

    /// Transformation applies to point positions
    ///
    /// TODO(andreas): Since we blindly apply this to positions only there is no restriction on this matrix.
    /// TODO(andreas): We don't apply scaling to the radius yet. Need to pass a scaling factor like this in
    /// `let scale = Mat3::from(world_from_obj).determinant().abs().cbrt()`
    pub world_from_obj: glam::Mat4,

    /// Additional properties of this point cloud batch.
    pub flags: PointCloudBatchFlags,

    /// Number of points covered by this batch.
    ///
    /// The batch will start with the next point after the one the previous batch ended with.
    pub point_count: u32,
}

/// Description of a point cloud.
pub struct PointCloudVertex {
    /// Connected points. Must be at least 2.
    pub position: glam::Vec3,

    /// Radius of the point in world space
    pub radius: Size,
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum PointCloudDrawDataError {
    #[error("Size of vertex & color array was not equal")]
    NumberOfColorsNotEqualNumberOfVertices,
}

/// Textures are 2D since 1D textures are very limited in size (8k typically).
/// Need to keep this value in sync with `point_cloud.wgsl`!
/// We store `vec4<f32> + [u8;4]` = 20 bytes per texel.
const DATA_TEXTURE_SIZE: u32 = 2048; // 2ki x 2ki = 4 Mi = 80 MiB

impl PointCloudDrawData {
    /// Maximum number of vertices per [`PointCloudDrawData`].
    ///
    /// TODO(#957): Get rid of this limit!.
    pub const MAX_NUM_POINTS: usize = (DATA_TEXTURE_SIZE * DATA_TEXTURE_SIZE) as usize;

    /// Transforms and uploads point cloud data to be consumed by gpu.
    ///
    /// Try to bundle all points into a single draw data instance whenever possible.
    /// Number of vertices and colors has to be equal.
    ///
    /// If no batches are passed, all points are assumed to be in a single batch with identity transform.
    pub fn new(
        ctx: &mut RenderContext,
        vertices: &[PointCloudVertex],
        colors: &[Color32],
        batches: &[PointCloudBatchInfo],
    ) -> Result<Self, PointCloudDrawDataError> {
        crate::profile_function!();

        let point_renderer = ctx.renderers.get_or_create::<_, PointCloudRenderer>(
            &ctx.shared_renderer_data,
            &mut ctx.gpu_resources,
            &ctx.device,
            &mut ctx.resolver,
        );

        if vertices.is_empty() {
            return Ok(PointCloudDrawData {
                bind_group_all_points: None,
                batches: Vec::new(),
            });
        }

        let fallback_batches = [PointCloudBatchInfo {
            label: "all points".into(),
            world_from_obj: glam::Mat4::IDENTITY,
            flags: PointCloudBatchFlags::empty(),
            point_count: vertices.len() as _,
        }];
        let batches = if batches.is_empty() {
            &fallback_batches
        } else {
            batches
        };

        // Make sure the size of a row is a multiple of the row byte alignment to make buffer copies easier.
        static_assertions::const_assert_eq!(
            DATA_TEXTURE_SIZE * std::mem::size_of::<gpu_data::PositionData>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );
        static_assertions::const_assert_eq!(
            DATA_TEXTURE_SIZE * std::mem::size_of::<[u8; 4]>() as u32
                % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT,
            0
        );

        if vertices.len() != colors.len() {
            return Err(PointCloudDrawDataError::NumberOfColorsNotEqualNumberOfVertices);
        }

        let (vertices, colors) = if vertices.len() >= Self::MAX_NUM_POINTS {
            re_log::error_once!(
                "Reached maximum number of supported points. Clamping down to {}, passed were {}.
 See also https://github.com/rerun-io/rerun/issues/957",
                Self::MAX_NUM_POINTS,
                vertices.len()
            );
            (
                &vertices[..Self::MAX_NUM_POINTS],
                &colors[..Self::MAX_NUM_POINTS],
            )
        } else {
            (vertices, colors)
        };

        // TODO(andreas): We want a "stack allocation" here that lives for one frame.
        //                  Note also that this doesn't protect against sharing the same texture with several PointDrawData!
        let position_data_texture_desc = TextureDesc {
            label: "point cloud position data".into(),
            size: wgpu::Extent3d {
                width: DATA_TEXTURE_SIZE,
                height: DATA_TEXTURE_SIZE,
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
        let num_points_written =
            wgpu::util::align_to(vertices.len() as u32, DATA_TEXTURE_SIZE) as usize;
        let num_points_zeroed = num_points_written - vertices.len();
        let position_and_size_staging = {
            crate::profile_scope!("collect_pos_size");
            vertices
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
            colors
                .iter()
                .cloned()
                .chain(std::iter::repeat(Color32::TRANSPARENT).take(num_points_zeroed))
                .collect_vec()
        };

        // Upload data from staging buffers to gpu.
        let size = wgpu::Extent3d {
            width: DATA_TEXTURE_SIZE,
            height: num_points_written as u32 / DATA_TEXTURE_SIZE,
            depth_or_array_layers: 1,
        };

        {
            crate::profile_scope!("write_pos_size_texture");
            ctx.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &ctx
                        .gpu_resources
                        .textures
                        .get_resource(&position_data_texture)
                        .unwrap()
                        .texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&position_and_size_staging),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new(
                        DATA_TEXTURE_SIZE * std::mem::size_of::<gpu_data::PositionData>() as u32,
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
                        .get_resource(&color_texture)
                        .unwrap()
                        .texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&color_staging),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: NonZeroU32::new(
                        DATA_TEXTURE_SIZE * std::mem::size_of::<[u8; 4]>() as u32,
                    ),
                    rows_per_image: None,
                },
                size,
            );
        }

        let bind_group_all_points = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &BindGroupDesc {
                label: "line drawdata".into(),
                entries: smallvec![
                    BindGroupEntry::DefaultTextureView(*position_data_texture),
                    BindGroupEntry::DefaultTextureView(*color_texture),
                ],
                layout: point_renderer.bind_group_layout_all_points,
            },
            &ctx.gpu_resources.bind_group_layouts,
            &ctx.gpu_resources.textures,
            &ctx.gpu_resources.buffers,
            &ctx.gpu_resources.samplers,
        );

        // Process batches
        let mut batches_internal = Vec::with_capacity(batches.len());
        {
            let allocation_size_per_uniform_buffer =
                uniform_buffer_allocation_size::<gpu_data::BatchUniformBuffer>(&ctx.device);
            let combined_buffers_size = allocation_size_per_uniform_buffer * batches.len() as u64;
            let uniform_buffers_handle = ctx.gpu_resources.buffers.alloc(
                &ctx.device,
                &BufferDesc {
                    label: "point batch uniform buffers".into(),
                    size: combined_buffers_size,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                },
            );

            let mut staging_buffer = ctx
                .queue
                .write_buffer_with(
                    ctx.gpu_resources
                        .buffers
                        .get_resource(&uniform_buffers_handle)
                        .unwrap(),
                    0,
                    NonZeroU64::new(combined_buffers_size).unwrap(),
                )
                .unwrap(); // Fails only if mapping is bigger than buffer size.

            let mut start_point_for_next_batch = 0;
            for (i, batch_info) in batches.iter().enumerate() {
                // CAREFUL: Memory from `write_buffer_with` may not be aligned, causing bytemuck to fail at runtime if we use it to cast the memory to a slice!
                let offset = i * allocation_size_per_uniform_buffer as usize;
                staging_buffer
                    [offset..(offset + std::mem::size_of::<gpu_data::BatchUniformBuffer>())]
                    .copy_from_slice(bytemuck::bytes_of(&gpu_data::BatchUniformBuffer {
                        world_from_obj: batch_info.world_from_obj.into(),
                        flags: batch_info.flags,
                        _padding: glam::Vec3::ZERO,
                    }));

                let bind_group = ctx.gpu_resources.bind_groups.alloc(
                    &ctx.device,
                    &BindGroupDesc {
                        label: batch_info.label.clone(),
                        entries: smallvec![BindGroupEntry::Buffer {
                            handle: *uniform_buffers_handle,
                            offset: offset as _,
                            size: NonZeroU64::new(
                                std::mem::size_of::<gpu_data::BatchUniformBuffer>() as _
                            ),
                        }],
                        layout: point_renderer.bind_group_layout_batch,
                    },
                    &ctx.gpu_resources.bind_group_layouts,
                    &ctx.gpu_resources.textures,
                    &ctx.gpu_resources.buffers,
                    &ctx.gpu_resources.samplers,
                );

                let point_vertex_range_end = (start_point_for_next_batch + batch_info.point_count)
                    .min(Self::MAX_NUM_POINTS as u32);

                batches_internal.push(PointCloudBatch {
                    bind_group,
                    vertex_range: (start_point_for_next_batch * 6)
                        ..((start_point_for_next_batch + batch_info.point_count) * 6),
                });

                start_point_for_next_batch = point_vertex_range_end;

                // Should happen only if the number of vertices was clamped.
                if start_point_for_next_batch >= vertices.len() as u32 {
                    break;
                }
            }
        }

        Ok(PointCloudDrawData {
            bind_group_all_points: Some(bind_group_all_points),
            batches: batches_internal,
        })
    }
}

pub struct PointCloudRenderer {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout_all_points: GpuBindGroupLayoutHandle,
    bind_group_layout_batch: GpuBindGroupLayoutHandle,
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

        let bind_group_layout_all_points = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "point cloud - all".into(),
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

        let bind_group_layout_batch = pools.bind_group_layouts.get_or_create(
            device,
            &BindGroupLayoutDesc {
                label: "point cloud - batch".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(std::mem::size_of::<
                            gpu_data::BatchUniformBuffer,
                        >() as _),
                    },
                    count: None,
                }],
            },
        );

        let pipeline_layout = pools.pipeline_layouts.get_or_create(
            device,
            &PipelineLayoutDesc {
                label: "point cloud".into(),
                entries: vec![
                    shared_data.global_bindings.layout,
                    bind_group_layout_all_points,
                    bind_group_layout_batch,
                ],
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
            bind_group_layout_all_points,
            bind_group_layout_batch,
        }
    }

    fn draw<'a>(
        &self,
        pools: &'a WgpuResourcePools,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &Self::RendererDrawData,
    ) -> anyhow::Result<()> {
        let Some(bind_group_all_points) = &draw_data.bind_group_all_points else {
            return Ok(()); // No points submitted.
        };
        let bind_group_line_data = pools.bind_groups.get_resource(bind_group_all_points)?;
        let pipeline = pools.render_pipelines.get_resource(self.render_pipeline)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, bind_group_line_data, &[]);

        for batch in &draw_data.batches {
            pass.set_bind_group(2, pools.bind_groups.get_resource(&batch.bind_group)?, &[]);
            pass.draw(batch.vertex_range.clone(), 0..1);
        }

        Ok(())
    }
}
