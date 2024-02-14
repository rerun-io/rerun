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

use std::{num::NonZeroU64, ops::Range};

use crate::{
    allocator::create_and_fill_uniform_buffer_batch,
    draw_phases::{DrawPhase, OutlineMaskProcessor, PickingLayerObjectId, PickingLayerProcessor},
    include_shader_module,
    renderer::data_texture_desc,
    wgpu_resources::GpuRenderPipelinePoolAccessor,
    DebugLabel, DepthOffset, OutlineMaskPreference, PointCloudBuilder,
};
use bitflags::bitflags;
use bytemuck::Zeroable as _;
use enumset::{enum_set, EnumSet};
use itertools::Itertools as _;
use smallvec::smallvec;

use crate::{
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, PipelineLayoutDesc, RenderPipelineDesc,
    },
};

use super::{DrawData, DrawError, RenderContext, Renderer};

bitflags! {
    /// Property flags for a point batch
    ///
    /// Needs to be kept in sync with `point_cloud.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PointCloudBatchFlags : u32 {
        /// If true, we shade all points in the batch like spheres.
        const FLAG_ENABLE_SHADING = 0b0001;

        /// If true, draw 2D camera facing circles instead of spheres.
        const FLAG_DRAW_AS_CIRCLES = 0b0010;
    }
}

mod gpu_data {
    use crate::{draw_phases::PickingLayerObjectId, wgpu_buffer_types, Size};

    // Don't use `wgsl_buffer_types` since this data doesn't go into a buffer, so alignment rules don't apply like on buffers..

    /// Position and radius.
    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct PositionRadius {
        pub pos: glam::Vec3,

        /// Radius of the point in world space
        pub radius: Size, // Might use a f16 here to free memory for more data!
    }
    static_assertions::assert_eq_size!(PositionRadius, glam::Vec4);

    /// Uniform buffer that changes once per draw data rendering.
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct DrawDataUniformBuffer {
        pub radius_boost_in_ui_points: wgpu_buffer_types::F32RowPadded,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 1],
    }

    /// Uniform buffer that changes for every batch of points.
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct BatchUniformBuffer {
        pub world_from_obj: wgpu_buffer_types::Mat4,

        pub flags: u32, // PointCloudBatchFlags
        pub depth_offset: f32,
        pub _row_padding: [f32; 2],

        pub outline_mask_ids: wgpu_buffer_types::UVec2,
        pub picking_object_id: PickingLayerObjectId,

        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 6],
    }
}

/// Internal, ready to draw representation of [`PointCloudBatchInfo`]
#[derive(Clone)]
struct PointCloudBatch {
    bind_group: GpuBindGroup,
    vertex_range: Range<u32>,
    active_phases: EnumSet<DrawPhase>,
}

/// A point cloud drawing operation.
/// Expected to be recrated every frame.
#[derive(Clone)]
pub struct PointCloudDrawData {
    bind_group_all_points: Option<GpuBindGroup>,
    bind_group_all_points_outline_mask: Option<GpuBindGroup>,
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
    /// TODO(andreas): We don't apply scaling to the radius yet. Need to pass a scaling factor like this in
    /// `let scale = Mat3::from(world_from_obj).determinant().abs().cbrt()`
    pub world_from_obj: glam::Affine3A,

    /// Additional properties of this point cloud batch.
    pub flags: PointCloudBatchFlags,

    /// Number of points covered by this batch.
    ///
    /// The batch will start with the next point after the one the previous batch ended with.
    pub point_count: u32,

    /// Optional outline mask setting for the entire batch.
    pub overall_outline_mask_ids: OutlineMaskPreference,

    /// Defines an outline mask for an individual vertex ranges.
    ///
    /// Vertex ranges are relative within the current batch.
    ///
    /// Having many of these individual outline masks can be slow as they require each their own uniform buffer & draw call.
    /// This feature is meant for a limited number of "extra selections"
    /// If an overall mask is defined as well, the per-point-range masks is overwriting the overall mask.
    pub additional_outline_mask_ids_vertex_ranges: Vec<(Range<u32>, OutlineMaskPreference)>,

    /// Picking object id that applies for the entire batch.
    pub picking_object_id: PickingLayerObjectId,

    /// Depth offset applied after projection.
    pub depth_offset: DepthOffset,
}

pub use gpu_data::PositionRadius;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum PointCloudDrawDataError {
    #[error("Failed to transfer data to the GPU: {0}")]
    FailedTransferringDataToGpu(#[from] crate::allocator::CpuWriteGpuReadError),
}

impl PointCloudDrawData {
    pub const COLOR_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
    pub const POSITION_DATA_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
    pub const PICKING_INSTANCE_ID_TEXTURE_FORMAT: wgpu::TextureFormat =
        wgpu::TextureFormat::Rg32Uint;

    /// Transforms and uploads point cloud data to be consumed by gpu.
    ///
    /// Try to bundle all points into a single draw data instance whenever possible.
    /// Number of vertices and colors has to be equal.
    ///
    /// If no batches are passed, all points are assumed to be in a single batch with identity transform.
    pub fn new(
        ctx: &RenderContext,
        mut builder: PointCloudBuilder,
    ) -> Result<Self, PointCloudDrawDataError> {
        re_tracing::profile_function!();

        let point_renderer = ctx.renderer::<PointCloudRenderer>();

        let vertices = builder.vertices.as_slice();
        let batches = builder.batches.as_slice();

        if vertices.is_empty() {
            return Ok(PointCloudDrawData {
                bind_group_all_points: None,
                bind_group_all_points_outline_mask: None,
                batches: Vec::new(),
            });
        }

        let fallback_batches = [PointCloudBatchInfo {
            label: "fallback_batches".into(),
            world_from_obj: glam::Affine3A::IDENTITY,
            flags: PointCloudBatchFlags::empty(),
            point_count: vertices.len() as _,
            overall_outline_mask_ids: OutlineMaskPreference::NONE,
            additional_outline_mask_ids_vertex_ranges: Vec::new(),
            picking_object_id: Default::default(),
            depth_offset: 0,
        }];
        let batches = if batches.is_empty() {
            &fallback_batches
        } else {
            batches
        };

        // Points are stored on a 2d texture (we can't use buffers due to WebGL compatibility).
        // 2D texture size limits on desktop is at least 8192x8192.
        // Android WebGL is known to only support 4096x4096, see https://web3dsurvey.com/webgl2/parameters/MAX_TEXTURE_SIZE
        // Android WebGPU, however almost always supports 8192x8192 and most of the time 16384x16384, see https://web3dsurvey.com/webgpu/limits/maxTextureDimension2D
        // => Even with a conservative 4096x4096, we can store 16 million points on a single texture, so we're very unlikely to ever hit this.
        // (and the typ typical 16384x16384 gives us a max 268 million points, far more than we can realistically render with this renderer)
        let max_texture_dimension_2d = ctx.device.limits().max_texture_dimension_2d;
        let max_num_points = max_texture_dimension_2d as usize * max_texture_dimension_2d as usize;
        let vertices = if vertices.len() > max_num_points {
            re_log::error_once!(
                "Reached maximum number of supported points. Clamping down to {}, passed were {}.",
                max_num_points,
                vertices.len()
            );
            &vertices[..max_num_points]
        } else {
            vertices
        };

        let position_data_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &data_texture_desc(
                "PointCloudDrawData::position_data_texture",
                Self::POSITION_DATA_TEXTURE_FORMAT,
                vertices.len() as u32,
                max_texture_dimension_2d,
            ),
        );
        let color_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &data_texture_desc(
                "PointCloudDrawData::color_texture",
                Self::COLOR_TEXTURE_FORMAT,
                vertices.len() as u32,
                max_texture_dimension_2d,
            ),
        );

        let picking_instance_id_texture = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &data_texture_desc(
                "PointCloudDrawData::picking_instance_id_texture",
                Self::PICKING_INSTANCE_ID_TEXTURE_FORMAT,
                vertices.len() as u32,
                max_texture_dimension_2d,
            ),
        );

        {
            re_tracing::profile_scope!("write_pos_size_texture");

            let texture_size = position_data_texture.texture.size();
            let texel_count = (texture_size.width * texture_size.height) as usize;
            let num_elements_padding = texel_count - vertices.len();

            let mut staging_buffer = ctx.cpu_write_gpu_read_belt.lock().allocate(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                texel_count,
            )?;
            staging_buffer.extend_from_slice(vertices)?;
            staging_buffer.fill_n(gpu_data::PositionRadius::zeroed(), num_elements_padding)?;
            staging_buffer.copy_to_texture2d(
                ctx.active_frame.before_view_builder_encoder.lock().get(),
                wgpu::ImageCopyTexture {
                    texture: &position_data_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                position_data_texture.texture.size(),
            )?;
        }
        {
            let texture_size = color_texture.texture.size();
            let texel_count = (texture_size.width * texture_size.height) as usize;
            let num_elements_padding = texel_count - vertices.len();

            builder
                .color_buffer
                .fill_n(ecolor::Color32::TRANSPARENT, num_elements_padding)?;
            builder.color_buffer.copy_to_texture2d_entire_first_layer(
                ctx.active_frame.before_view_builder_encoder.lock().get(),
                &color_texture,
            )?;
        }

        {
            let texture_size = picking_instance_id_texture.texture.size();
            let texel_count = (texture_size.width * texture_size.height) as usize;
            let num_elements_padding = texel_count - vertices.len();

            builder
                .picking_instance_ids_buffer
                .fill_n(Default::default(), num_elements_padding)?;
            builder
                .picking_instance_ids_buffer
                .copy_to_texture2d_entire_first_layer(
                    ctx.active_frame.before_view_builder_encoder.lock().get(),
                    &picking_instance_id_texture,
                )?;
        }

        let draw_data_uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
            ctx,
            "PointCloudDrawData::DrawDataUniformBuffer".into(),
            [
                gpu_data::DrawDataUniformBuffer {
                    radius_boost_in_ui_points: 0.0.into(),
                    end_padding: Default::default(),
                },
                gpu_data::DrawDataUniformBuffer {
                    radius_boost_in_ui_points: builder
                        .radius_boost_in_ui_points_for_outlines
                        .into(),
                    end_padding: Default::default(),
                },
            ]
            .into_iter(),
        );
        let (draw_data_uniform_buffer_bindings_normal, draw_data_uniform_buffer_bindings_outline) =
            draw_data_uniform_buffer_bindings
                .into_iter()
                .collect_tuple()
                .unwrap();

        let mk_bind_group = |label, draw_data_uniform_buffer_binding| {
            ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label,
                    entries: smallvec![
                        BindGroupEntry::DefaultTextureView(position_data_texture.handle),
                        BindGroupEntry::DefaultTextureView(color_texture.handle),
                        BindGroupEntry::DefaultTextureView(picking_instance_id_texture.handle),
                        draw_data_uniform_buffer_binding,
                    ],
                    layout: point_renderer.bind_group_layout_all_points,
                },
            )
        };

        let bind_group_all_points = mk_bind_group(
            "PointCloudDrawData::bind_group_all_points".into(),
            draw_data_uniform_buffer_bindings_normal,
        );
        let bind_group_all_points_outline_mask = mk_bind_group(
            "PointCloudDrawData::bind_group_all_points_outline_mask".into(),
            draw_data_uniform_buffer_bindings_outline,
        );

        // Process batches
        let mut batches_internal = Vec::with_capacity(batches.len());
        {
            let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
                ctx,
                "point batch uniform buffers".into(),
                batches
                    .iter()
                    .map(|batch_info| gpu_data::BatchUniformBuffer {
                        world_from_obj: batch_info.world_from_obj.into(),
                        flags: batch_info.flags.bits(),
                        outline_mask_ids: batch_info
                            .overall_outline_mask_ids
                            .0
                            .unwrap_or_default()
                            .into(),
                        picking_object_id: batch_info.picking_object_id,
                        depth_offset: batch_info.depth_offset as f32,

                        _row_padding: [0.0, 0.0],
                        end_padding: Default::default(),
                    }),
            );

            // Generate additional "micro batches" for each point range that has a unique outline setting.
            // This is fairly costly if there's many, but easy and low-overhead if there's only few, which is usually what we expect!
            let mut uniform_buffer_bindings_mask_only_batches =
                create_and_fill_uniform_buffer_batch(
                    ctx,
                    "lines batch uniform buffers - mask only".into(),
                    batches
                        .iter()
                        .flat_map(|batch_info| {
                            batch_info
                                .additional_outline_mask_ids_vertex_ranges
                                .iter()
                                .map(|(_, mask)| gpu_data::BatchUniformBuffer {
                                    world_from_obj: batch_info.world_from_obj.into(),
                                    flags: batch_info.flags.bits(),
                                    outline_mask_ids: mask.0.unwrap_or_default().into(),
                                    picking_object_id: batch_info.picking_object_id,
                                    depth_offset: batch_info.depth_offset as f32,

                                    _row_padding: [0.0, 0.0],
                                    end_padding: Default::default(),
                                })
                        })
                        .collect::<Vec<_>>()
                        .into_iter(),
                )
                .into_iter();

            let mut start_point_for_next_batch = 0;
            for (batch_info, uniform_buffer_binding) in
                batches.iter().zip(uniform_buffer_bindings.into_iter())
            {
                let point_vertex_range_end = (start_point_for_next_batch + batch_info.point_count)
                    .min(max_num_points as u32);
                let mut active_phases = enum_set![DrawPhase::Opaque | DrawPhase::PickingLayer];
                // Does the entire batch participate in the outline mask phase?
                if batch_info.overall_outline_mask_ids.is_some() {
                    active_phases.insert(DrawPhase::OutlineMask);
                }

                batches_internal.push(point_renderer.create_point_cloud_batch(
                    ctx,
                    batch_info.label.clone(),
                    uniform_buffer_binding,
                    start_point_for_next_batch..point_vertex_range_end,
                    active_phases,
                ));

                for (range, _) in &batch_info.additional_outline_mask_ids_vertex_ranges {
                    let range = (range.start + start_point_for_next_batch)
                        ..(range.end + start_point_for_next_batch);
                    batches_internal.push(point_renderer.create_point_cloud_batch(
                        ctx,
                        format!("{:?} strip-only {:?}", batch_info.label, range).into(),
                        uniform_buffer_bindings_mask_only_batches.next().unwrap(),
                        range.clone(),
                        enum_set![DrawPhase::OutlineMask],
                    ));
                }

                start_point_for_next_batch = point_vertex_range_end;

                // Should happen only if the number of vertices was clamped.
                if start_point_for_next_batch >= vertices.len() as u32 {
                    break;
                }
            }
        }

        Ok(PointCloudDrawData {
            bind_group_all_points: Some(bind_group_all_points),
            bind_group_all_points_outline_mask: Some(bind_group_all_points_outline_mask),
            batches: batches_internal,
        })
    }
}

pub struct PointCloudRenderer {
    render_pipeline_color: GpuRenderPipelineHandle,
    render_pipeline_picking_layer: GpuRenderPipelineHandle,
    render_pipeline_outline_mask: GpuRenderPipelineHandle,
    bind_group_layout_all_points: GpuBindGroupLayoutHandle,
    bind_group_layout_batch: GpuBindGroupLayoutHandle,
}

impl PointCloudRenderer {
    fn create_point_cloud_batch(
        &self,
        ctx: &RenderContext,
        label: DebugLabel,
        uniform_buffer_binding: BindGroupEntry,
        vertex_range: Range<u32>,
        active_phases: EnumSet<DrawPhase>,
    ) -> PointCloudBatch {
        // TODO(andreas): There should be only a single bindgroup with dynamic indices for all batches.
        //                  (each batch would then know which dynamic indices to use in the bindgroup)
        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label,
                entries: smallvec![uniform_buffer_binding],
                layout: self.bind_group_layout_batch,
            },
        );

        PointCloudBatch {
            bind_group,
            vertex_range: (vertex_range.start * 6)..(vertex_range.end * 6),
            active_phases,
        }
    }
}

impl Renderer for PointCloudRenderer {
    type RendererDrawData = PointCloudDrawData;

    fn participated_phases() -> &'static [DrawPhase] {
        &[
            DrawPhase::OutlineMask,
            DrawPhase::Opaque,
            DrawPhase::PickingLayer,
        ]
    }

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let render_pipelines = &ctx.gpu_resources.render_pipelines;

        let bind_group_layout_all_points = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "PointCloudRenderer::bind_group_layout_all_points".into(),
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(std::mem::size_of::<
                                gpu_data::DrawDataUniformBuffer,
                            >() as _),
                        },
                        count: None,
                    },
                ],
            },
        );

        let bind_group_layout_batch = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "PointCloudRenderer::bind_group_layout_batch".into(),
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

        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "PointCloudRenderer::pipeline_layout".into(),
                entries: vec![
                    ctx.global_bindings.layout,
                    bind_group_layout_all_points,
                    bind_group_layout_batch,
                ],
            },
        );

        let shader_module_desc = include_shader_module!("../../shader/point_cloud.wgsl");
        let shader_module = ctx
            .gpu_resources
            .shader_modules
            .get_or_create(ctx, &shader_module_desc);

        // WORKAROUND for https://github.com/gfx-rs/naga/issues/1743
        let mut shader_module_desc_vertex = shader_module_desc.clone();
        shader_module_desc_vertex.extra_workaround_replacements =
            vec![("fwidth(".to_owned(), "f32(".to_owned())];
        let shader_module_vertex = ctx
            .gpu_resources
            .shader_modules
            .get_or_create(ctx, &shader_module_desc_vertex);

        let render_pipeline_desc_color = RenderPipelineDesc {
            label: "PointCloudRenderer::render_pipeline_color".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module_vertex,
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
                // We discard pixels to do the round cutout, therefore we need to calculate
                // our own sampling mask.
                alpha_to_coverage_enabled: true,
                ..ViewBuilder::MAIN_TARGET_DEFAULT_MSAA_STATE
            },
        };
        let render_pipeline_color =
            render_pipelines.get_or_create(ctx, &render_pipeline_desc_color);
        let render_pipeline_picking_layer = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "PointCloudRenderer::render_pipeline_picking_layer".into(),
                fragment_entrypoint: "fs_main_picking_layer".into(),
                render_targets: smallvec![Some(PickingLayerProcessor::PICKING_LAYER_FORMAT.into())],
                depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                ..render_pipeline_desc_color.clone()
            },
        );
        let render_pipeline_outline_mask = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "PointCloudRenderer::render_pipeline_outline_mask".into(),
                fragment_entrypoint: "fs_main_outline_mask".into(),
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                // Alpha to coverage doesn't work with the mask integer target.
                multisample: OutlineMaskProcessor::mask_default_msaa_state(&ctx.config.device_caps),
                ..render_pipeline_desc_color
            },
        );

        PointCloudRenderer {
            render_pipeline_color,
            render_pipeline_picking_layer,
            render_pipeline_outline_mask,
            bind_group_layout_all_points,
            bind_group_layout_batch,
        }
    }

    fn draw<'a>(
        &self,
        render_pipelines: &'a GpuRenderPipelinePoolAccessor<'a>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
        draw_data: &'a Self::RendererDrawData,
    ) -> Result<(), DrawError> {
        let (pipeline_handle, bind_group_all_points) = match phase {
            DrawPhase::OutlineMask => (
                self.render_pipeline_outline_mask,
                &draw_data.bind_group_all_points_outline_mask,
            ),
            DrawPhase::Opaque => (self.render_pipeline_color, &draw_data.bind_group_all_points),
            DrawPhase::PickingLayer => (
                self.render_pipeline_picking_layer,
                &draw_data.bind_group_all_points,
            ),
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };
        let Some(bind_group_all_points) = bind_group_all_points else {
            return Ok(()); // No points submitted.
        };
        let pipeline = render_pipelines.get(pipeline_handle)?;

        pass.set_pipeline(pipeline);
        pass.set_bind_group(1, bind_group_all_points, &[]);

        for batch in &draw_data.batches {
            if batch.active_phases.contains(phase) {
                pass.set_bind_group(2, &batch.bind_group, &[]);
                pass.draw(batch.vertex_range.clone(), 0..1);
            }
        }

        Ok(())
    }
}
